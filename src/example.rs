use ethers::types::Address;
use ethers::types::U256;
use good_lp::{
    default_solver,
    solvers::{Solution, SolverModel},
    variables, Expression, ProblemVariables, Solution as LpSolution, Variable,
};
use petgraph::algo::dijkstra;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Result;
use std::io::Write;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pools {
    pools: Vec<UniV2Pool>,
}

impl Pools {
    pub fn load_from_file(file_path: &str) -> Result<Vec<UniV2Pool>> {
        let file = File::open(file_path)?;
        let reader = std::io::BufReader::new(file);
        let storage: Pools = serde_json::from_reader(reader)?;
        Ok(storage.pools)
    }

    pub fn calculate_profit(&self, input_amount: f64) -> f64 {
        let mut amount = input_amount;

        for pool in &self.pools {
            let reserve0 = pool.reserve0.as_u128() as f64;
            let reserve1 = pool.reserve1.as_u128() as f64;
            let fee = 1.0 - (pool.router_fee.as_u64() as f64 / 10000.0);

            // Exact constant product formula
            amount = (reserve1 * amount * fee) / (reserve0 + amount * fee);
        }

        amount // Return final amount after all swaps
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniV2Pool {
    pub address: Address,

    pub token0: Address,
    pub token1: Address,

    pub reserve0: U256,
    pub reserve1: U256,

    // router fee
    pub router_fee: U256,
    //  token tax when token0 is in
    pub fees0: U256,
    //  token tax when token1 is in
    pub fees1: U256,
}

pub fn get_pools() -> Vec<UniV2Pool> {
    let storage = Pools::load_from_file("./caaaamelot.json").expect("Failed on loading data");
    print!("pool 1  {:?}", storage[0]);
    storage
}

pub fn build_graph(pools: &Vec<UniV2Pool>) -> HashMap<Address, Vec<(usize, Address)>> {
    let mut graph: HashMap<Address, Vec<(usize, Address)>> = HashMap::new();

    // For each pool
    for (pool_idx, pool) in pools.iter().enumerate() {
        // Add token0 -> token1 edge
        graph
            .entry(pool.token0.clone())
            .or_default()
            .push((pool_idx, pool.token1.clone()));

        // Add token1 -> token0 edge
        graph
            .entry(pool.token1.clone())
            .or_default()
            .push((pool_idx, pool.token0.clone()));
    }

    graph
}

pub struct PoolGraph {
    graph: Graph<Address, u64>,
    token_map: HashMap<Address, NodeIndex>,
}

impl PoolGraph {
    pub fn new(pools: &Vec<UniV2Pool>) -> Self {
        let mut graph: Graph<ethers::types::H160, u64> = Graph::new();
        let mut token_map = HashMap::new();

        // Create nodes
        for pool in pools {
            if !token_map.contains_key(&pool.token0) {
                let idx = graph.add_node(pool.token0.clone());
                token_map.insert(pool.token0.clone(), idx);
            }
            if !token_map.contains_key(&pool.token1) {
                let idx = graph.add_node(pool.token1.clone());
                token_map.insert(pool.token1.clone(), idx);
            }
        }

        // Add edges
        for pool in pools {
            let n1 = token_map[&pool.token0];
            let n2 = token_map[&pool.token1];
            graph.add_edge(n1, n2, 0);
            graph.add_edge(n2, n1, 0);
        }

        PoolGraph { graph, token_map }
    }

    pub fn detect_cycles(&self, start: Address) -> Vec<Vec<Address>> {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut cycles = Vec::new();

        if let Some(&start_idx) = self.token_map.get(&start) {
            // Initialize with start node
            stack.push(start_idx);
            visited.insert(start_idx);

            self.dfs_cycle_detection(start_idx, start_idx, &mut visited, &mut stack, &mut cycles);
        }

        // Filter valid cycles
        cycles
            .iter()
            .map(|cycle| {
                cycle
                    .iter()
                    .map(|&idx| self.graph[idx].clone())
                    .collect::<Vec<Address>>()
            })
            .filter(|cycle| {
                // Must start with start token
                if cycle.is_empty() || cycle[0] != start {
                    return false;
                }

                // Check for duplicates except start/end
                let inner_tokens = &cycle[1..cycle.len() - 1];
                let unique_tokens: HashSet<_> = inner_tokens.iter().collect();
                if unique_tokens.len() != inner_tokens.len() {
                    return false;
                }

                true
            })
            .collect()
    }

    fn dfs_cycle_detection(
        &self,
        node: NodeIndex,
        start_idx: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        stack: &mut Vec<NodeIndex>,
        cycles: &mut Vec<Vec<NodeIndex>>,
    ) {
        for edge in self.graph.edges(node) {
            let neighbor = edge.target();

            // Found a cycle back to start
            if neighbor == start_idx && stack.len() > 2 {
                let mut cycle = stack.clone();
                cycle.push(start_idx);
                cycles.push(cycle);
                continue;
            }

            // Skip if already visited (unless it's start node)
            if neighbor != start_idx && visited.contains(&neighbor) {
                continue;
            }

            visited.insert(neighbor);
            stack.push(neighbor);
            self.dfs_cycle_detection(neighbor, start_idx, visited, stack, cycles);
            stack.pop();
            visited.remove(&neighbor);
        }
    }

    pub fn verify_path_exists(&self, path: &[Address], pools: &[UniV2Pool]) -> bool {
        for window in path.windows(2) {
            let from = window[0];
            let to = window[1];

            // Check if a pool exists for this pair
            let pool_exists = pools.iter().any(|p| {
                (p.token0 == from && p.token1 == to) || (p.token1 == from && p.token0 == to)
            });

            if !pool_exists {
                return false;
            }
        }

        // Check final connection back to start
        if let (Some(last), Some(first)) = (path.last(), path.first()) {
            pools.iter().any(|p| {
                (p.token0 == *last && p.token1 == *first)
                    || (p.token1 == *last && p.token0 == *first)
            })
        } else {
            false
        }
    }

    pub fn convert_cycles_to_pool_paths(
        &self,
        cycles: Vec<Vec<Address>>,
        pools: &[UniV2Pool],
    ) -> Vec<Vec<UniV2Pool>> {
        cycles
            .iter()
            .filter_map(|cycle| self.convert_single_cycle_to_pools(cycle, pools))
            .collect()
    }

    fn convert_single_cycle_to_pools(
        &self,
        cycle: &[Address],
        pools: &[UniV2Pool],
    ) -> Option<Vec<UniV2Pool>> {
        let mut pool_path = Vec::new();

        // Include the last->first connection in our windows by creating a circular path
        let mut circular_path = cycle.to_vec();
        if let Some(first) = cycle.first() {
            circular_path.push(*first);
        }

        // Convert each address pair to corresponding pool
        for window in circular_path.windows(2) {
            let from = window[0];
            let to = window[1];

            // Find the pool connecting these tokens
            let pool = pools.iter().find(|p| {
                (p.token0 == from && p.token1 == to) || (p.token1 == from && p.token0 == to)
            });

            match pool {
                Some(pool) => pool_path.push(pool.clone()),
                None => {
                    println!("Warning: No pool found for path {} -> {}", from, to);
                    return None; // Skip this cycle if we can't find a pool
                }
            }
        }

        if pool_path.is_empty() {
            None
        } else {
            Some(pool_path)
        }
    }

    pub fn find_arb(detected_cycles: Vec<Vec<Pools>>) {
        let mut problem = ProblemVariables::new();

        // Step 1: Define input variables for each path (0.0 to 1000.0)
        let x: Vec<Variable> = (0..detected_cycles.len())
            .map(|i| {
                problem.add(
                    good_lp::variable::variable()
                        .min(0.0)
                        .max(1000.0)
                        .name(format!("input_{}", i)),
                )
            })
            .collect();

        // Step 2: Define binary selection variables for each path (0 to 1)
        let z: Vec<Variable> = (0..detected_cycles.len())
            .map(|i| {
                problem.add(
                    good_lp::variable::variable()
                        .binary()
                        .name(format!("select_{}", i)),
                )
            })
            .collect();

        // Step 3: Define profit expressions for each path
        let profits: Vec<Expression> = detected_cycles
            .iter()
            .enumerate()
            .map(|(i, cycle)| {
                // Calculate the coefficient for this path
                let mut coefficient = 1.0;
                for pool in cycle {
                    let reserve0 = pool.pools[0].reserve0.as_u128() as f64;
                    let reserve1 = pool.pools[0].reserve1.as_u128() as f64;
                    let fee = 1.0 - (pool.pools[0].router_fee.as_u64() as f64 / 10000.0);
                    coefficient *= (reserve1 / reserve0) * fee;
                }

                // Calculate profit coefficient (output/input ratio - 1)
                let profit_coefficient = coefficient - 1.0;

                // First create the profit term: profit_coefficient * x[i]
                let mut profit_expr = Expression::from(x[i]) * profit_coefficient;

                // Then multiply by the binary selection variable
                // Using the add_mul method which is safer than direct multiplication
                let mut final_expr = Expression::default();
                final_expr.add_mul(1.0, profit_expr);
                final_expr.add_mul(1.0, z[i]);

                final_expr
            })
            .collect();

        // Create the optimization objective
        let objective = profits
            .iter()
            .fold(Expression::default(), |acc, expr| acc + expr.clone());

        // Constraint: sum of binary variables = 1
        let sum_z: Expression = z.iter().sum();

        // Constraint: limit total input amount
        let sum_x: Expression = x.iter().sum();

        // Build and solve the problem
        let solution = problem
            .maximise(objective)
            .using(default_solver)
            .with(sum_z.eq(1.0))
            .with(sum_x.leq(1000.0))
            .solve();

        // Handle the solution
        match solution {
            Ok(solution) => {
                for (i, &zi) in z.iter().enumerate() {
                    if solution.value(zi) > 0.5 {
                        let input_amount = solution.value(x[i]);
                        println!("Selected Path Index: {}", i);
                        println!("Optimal Input: {}", input_amount);

                        let mut current_amount = input_amount;

                        // Go through each pool in the path
                        for pools in &detected_cycles[i] {
                            for pool in &pools.pools {
                                let reserve0 = pool.reserve0.as_u128() as f64;
                                let reserve1 = pool.reserve1.as_u128() as f64;
                                let fee = 1.0 - (pool.router_fee.as_u64() as f64 / 10000.0);

                                // Apply constant product formula and fees
                                current_amount = (reserve1 * current_amount * fee)
                                    / (reserve0 + current_amount * fee);
                            }
                        }

                        let actual_profit = current_amount - input_amount;
                        println!("Actual Profit: {}", actual_profit);
                        if actual_profit > 0.0 {
                            println!("Found profitable arbitrage!");
                            println!("Input: {}", input_amount);
                            println!("Output: {}", current_amount);
                            println!("Profit: {}", actual_profit);
                        }
                    }
                }
            }
            Err(e) => println!("Failed to solve optimization problem: {:?}", e),
        }
    }
    pub fn print_cycle_details(&self, cycle: &[Address], pools: &[UniV2Pool]) {
        println!("\nCycle Details:");
        for window in cycle.windows(2) {
            let from = window[0];
            let to = window[1];

            // Find the connecting pool
            if let Some(pool) = pools.iter().find(|p| {
                (p.token0 == from && p.token1 == to) || (p.token1 == from && p.token0 == to)
            }) {
                println!(
                    "Token {} -> Token {}",
                    if pool.token0 == from { "0" } else { "1" },
                    if pool.token0 == to { "0" } else { "1" }
                );
                println!("  Pool Address: {}", pool.address);
                println!("  Reserves: {} / {}", pool.reserve0, pool.reserve1);
                println!("  Fee: {}", pool.router_fee);
            }
        }
    }

    pub fn export_visualization(&self, filename: &str) -> std::io::Result<()> {
        let mut file = File::create(filename)?;

        writeln!(file, "digraph {{")?;
        writeln!(file, "    layout=circo;")?; // Try circo layout
        writeln!(file, "    overlap=false;")?;
        writeln!(file, "    splines=true;")?;
        writeln!(file, "    node [shape=box];")?;

        let dot = format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        );
        let dot_content = dot.trim_start_matches("digraph {").trim_end_matches("}");
        writeln!(file, "{}", dot_content)?;

        writeln!(file, "}}")
    }
}

trait PathProfit {
    fn calculate_path_profit(&self, input_amount: f64) -> f64;
}

impl PathProfit for Vec<Pools> {
    fn calculate_path_profit(&self, mut amount: f64) -> f64 {
        for pools in self {
            for pool in &pools.pools {
                let reserve0 = pool.reserve0.as_u128() as f64;
                let reserve1 = pool.reserve1.as_u128() as f64;
                let fee = 1.0 - (pool.router_fee.as_u64() as f64 / 10000.0);

                // Apply constant product formula and fees
                amount = (reserve1 * amount * fee) / (reserve0 + amount * fee);
            }
        }
        amount
    }
}

fn main() {
    // Create instances of UniV2Pool
    let tokens = vec![
        Address::from([0x0; 20]),
        Address::from([0x1; 20]),
        Address::from([0x2; 20]),
        Address::from([0x3; 20]),
        Address::from([0x4; 20]),
        Address::from([0x5; 20]),
    ];

    // Updated pool addresses to avoid similarity with tokens
    let pools = vec![
        UniV2Pool {
            address: Address::from([0x10; 20]),
            token0: tokens[0],
            token1: tokens[1],
            reserve0: U256::from(1000),
            reserve1: U256::from(2000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x11; 20]),
            token0: tokens[1],
            token1: tokens[2],
            reserve0: U256::from(1500),
            reserve1: U256::from(2500),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x12; 20]),
            token0: tokens[2],
            token1: tokens[3],
            reserve0: U256::from(3000),
            reserve1: U256::from(1000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x13; 20]),
            token0: tokens[3],
            token1: tokens[4],
            reserve0: U256::from(2000),
            reserve1: U256::from(3000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x14; 20]),
            token0: tokens[4],
            token1: tokens[5],
            reserve0: U256::from(2500),
            reserve1: U256::from(3500),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x15; 20]),
            token0: tokens[5],
            token1: tokens[0], // Closing the loop back to token0
            reserve0: U256::from(3000),
            reserve1: U256::from(4000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x16; 20]),
            token0: tokens[0],
            token1: tokens[2], // Extra interlinking
            reserve0: U256::from(1500),
            reserve1: U256::from(1500),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x17; 20]),
            token0: tokens[1],
            token1: tokens[3], // Extra interlinking
            reserve0: U256::from(500),
            reserve1: U256::from(2000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x18; 20]),
            token0: tokens[3],
            token1: tokens[5], // Extra interlinking
            reserve0: U256::from(1000),
            reserve1: U256::from(3000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x19; 20]),
            token0: tokens[4],
            token1: tokens[1], // Extra interlinking
            reserve0: U256::from(2000),
            reserve1: U256::from(1000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
    ];

    // Create the PoolGraph from the pool instances
    let pool_graph = PoolGraph::new(&pools);
    let cycle = pool_graph.detect_cycles(tokens[3]);
    print!("cycle {:?}", cycle[2]);
    let path = pool_graph.convert_cycles_to_pool_paths(cycle, &pools);
    print!("path 1 {:?}", path[0]);
    pool_graph.export_visualization("only_usdc4.dot").unwrap();
}
