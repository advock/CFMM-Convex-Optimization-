use ethers::types::Address;
use ethers::types::U256;
use good_lp::{
    default_solver,
    solvers::{Solution, SolverModel},
    variables, Expression, ModelWithSOS1, ProblemVariables, Solution as LpSolution, Variable,
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
            amount = (reserve1 * amount * fee) / (reserve0 + amount * fee);
        }

        amount
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniV2Pool {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub router_fee: U256,
    pub fees0: U256,
    pub fees1: U256,
}

pub struct PoolGraph {
    graph: Graph<Address, u64>,
    token_map: HashMap<Address, NodeIndex>,
    pool_map: HashMap<(Address, Address), UniV2Pool>,
}

impl PoolGraph {
    pub fn new(pools: &Vec<UniV2Pool>) -> Self {
        let mut graph: Graph<Address, u64> = Graph::new();
        let mut token_map = HashMap::new();
        let mut pool_map = HashMap::new();

        // Create nodes
        for pool in pools {
            if !token_map.contains_key(&pool.token0) {
                let idx = graph.add_node(pool.token0);
                token_map.insert(pool.token0, idx);
            }
            if !token_map.contains_key(&pool.token1) {
                let idx = graph.add_node(pool.token1);
                token_map.insert(pool.token1, idx);
            }

            // Add to pool map - ensure tokens are in consistent order
            let key = if pool.token0 < pool.token1 {
                (pool.token0, pool.token1)
            } else {
                (pool.token1, pool.token0)
            };
            pool_map.insert(key, pool.clone());
        }

        // Add edges
        for pool in pools {
            let n1 = token_map[&pool.token0];
            let n2 = token_map[&pool.token1];
            graph.add_edge(n1, n2, 0);
            graph.add_edge(n2, n1, 0);
        }

        PoolGraph {
            graph,
            token_map,
            pool_map,
        }
    }

    pub fn get_pool(&self, token0: Address, token1: Address) -> Option<&UniV2Pool> {
        let key = if token0 < token1 {
            (token0, token1)
        } else {
            (token1, token0)
        };
        self.pool_map.get(&key)
    }

    pub fn detect_cycles(&self, start: Address) -> Vec<Vec<Address>> {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut cycles = Vec::new();

        if let Some(&start_idx) = self.token_map.get(&start) {
            stack.push(start_idx);
            visited.insert(start_idx);
            self.dfs_cycle_detection(start_idx, start_idx, &mut visited, &mut stack, &mut cycles);
        }

        cycles
            .iter()
            .map(|cycle| cycle.iter().map(|&idx| self.graph[idx].clone()).collect())
            .filter(|cycle: &Vec<Address>| {
                !cycle.is_empty() && cycle[0] == start && self.verify_path_exists(cycle)
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

            if neighbor == start_idx && stack.len() > 2 {
                let mut cycle = stack.clone();
                cycle.push(start_idx);
                cycles.push(cycle);
                continue;
            }

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

    pub fn verify_path_exists(&self, path: &[Address]) -> bool {
        //println!("\nVerifying path: {:?}", path);

        // Check consecutive pairs
        for window in path.windows(2) {
            //  println!("Checking pool between {} and {}", window[0], window[1]);
            if let Some(pool) = self.get_pool(window[0], window[1]) {
                // println!("  Found pool: {}", pool.address);
            } else {
                // println!("  No pool found between these tokens!");
                return false;
            }
        }

        // Check final connection
        if let (Some(last), Some(first)) = (path.last(), path.first()) {
            if (last == first) {
                return true;
            }
            //println!("Checking final connection: {} -> {}", last, first);
            let result = self.get_pool(*last, *first).is_some();
            // println!("  Final connection exists: {}", result);
            result
        } else {
            //println!("Invalid path: missing first or last token");
            false
        }
    }

    pub fn convert_cycle_to_pools(&self, cycle: &[Address]) -> Option<Vec<UniV2Pool>> {
        if !self.verify_path_exists(cycle) {
            return None;
        }

        let mut pools = Vec::new();
        let mut circular_path = cycle.to_vec();
        if let Some(first) = cycle.first().cloned() {
            circular_path.push(first);
        }

        for window in circular_path.windows(2) {
            if let Some(pool) = self.get_pool(window[0], window[1]) {
                pools.push(pool.clone());
            }
        }

        Some(pools)
    }

    pub fn print_cycle_details(&self, cycle: &[Address]) {
        println!("\nCycle Details:");

        let mut circular_path = cycle.to_vec();
        if let Some(first) = cycle.first().cloned() {
            circular_path.push(first);
        }

        for window in circular_path.windows(2) {
            if let Some(pool) = self.get_pool(window[0], window[1]) {
                println!("\nStep: {} -> {}", window[0], window[1]);
                println!("  Pool Address: {}", pool.address);
                println!("  Reserves: {} / {}", pool.reserve0, pool.reserve1);
                println!("  Router Fee: {} bps", pool.router_fee);
                println!("  Token Taxes: {} / {} bps", pool.fees0, pool.fees1);

                // Calculate price
                let (reserve_from, reserve_to) = if pool.token0 == window[0] {
                    (pool.reserve0, pool.reserve1)
                } else {
                    (pool.reserve1, pool.reserve0)
                };
                let price = reserve_to.as_u128() as f64 / reserve_from.as_u128() as f64;
                println!("  Price: {}", price);
            }
        }
    }

    pub fn export_visualization(&self, filename: &str) -> std::io::Result<()> {
        let mut file = File::create(filename)?;
        writeln!(file, "digraph {{")?;
        writeln!(file, "    layout=circo;")?;
        writeln!(file, "    overlap=false;")?;
        writeln!(file, "    splines=true;")?;
        writeln!(file, "    node [shape=box];")?;

        // Add nodes with token addresses
        for node_idx in self.graph.node_indices() {
            let token = &self.graph[node_idx];
            writeln!(file, "    {:?} [label=\"{}\"];", node_idx.index(), token)?;
        }

        // Add edges with pool information
        for edge in self.graph.edge_references() {
            let from = &self.graph[edge.source()];
            let to = &self.graph[edge.target()];
            if let Some(pool) = self.get_pool(*from, *to) {
                writeln!(
                    file,
                    "    {:?} -> {:?} [label=\"{}\"];",
                    edge.source().index(),
                    edge.target().index(),
                    pool.address
                )?;
            }
        }

        writeln!(file, "}}")
    }
    pub fn find_arb(&self, cycle_pools: Vec<Vec<UniV2Pool>>) -> Vec<(Vec<UniV2Pool>, f64, f64)> {
        println!("Analyzing {} potential cycles", cycle_pools.len());
        let mut profitable_paths = Vec::new();

        let mut problem: ProblemVariables = ProblemVariables::new();

        // Step 1: Define input variables for each path (0.0 to 1000.0)
        let x: Vec<Variable> = (0..cycle_pools.len())
            .map(|i| {
                problem.add(
                    good_lp::variable::variable()
                        .min(0.0)
                        .max(1156.0)
                        .name(format!("input_{}", i)),
                )
            })
            .collect();

        // Step 2: Define binary selection variables for each path (0 to 1)
        let z: Vec<Variable> = (0..cycle_pools.len())
            .map(|i| {
                problem.add(
                    good_lp::variable::variable()
                        .binary()
                        .name(format!("select_{}", i)),
                )
            })
            .collect();

        // Step 3: Define profit expressions for each path
        let profits: Vec<Expression> = cycle_pools
            .iter()
            .enumerate()
            .map(|(i, cycle)| {
                // Calculate the coefficient for this path
                let mut coefficient = 1.0;
                for pool in cycle {
                    let reserve0 = pool.reserve0.as_u128() as f64;
                    let reserve1 = pool.reserve1.as_u128() as f64;
                    let fee = 1.0 - (pool.router_fee.as_u64() as f64 / 10000.0);
                    coefficient *= (reserve1 / reserve0) * fee;
                }

                println!("\nAnalyzing cycle {}", i);
                println!("Path coefficient: {}", coefficient);

                let input_amount = Expression::from(x[i].clone());
                let selection = Expression::from(z[i].clone());

                // Calculate estimated output using the path coefficient
                let estimated_output = coefficient * input_amount.clone();

                // Profit is (output - input) * selection
                let mut profit = estimated_output - input_amount;

                profit.add_mul(1, selection);
                profit
            })
            .collect();

        // Create optimization objective
        let objective = profits
            .iter()
            .fold(Expression::default(), |acc, expr| acc + expr.clone());

        let sum_z: Expression = z.iter().sum();

        // Constraint: limit total input amount
        let sum_x: Expression = x.iter().sum();

        let solution = problem
            .maximise(objective)
            .using(default_solver)
            .with(sum_x.leq(1156.0))
            .with(sum_z.eq(52.0))
            .solve();

        print!("Raj theckrey");
        match solution {
            Ok(solution) => {
                for (i, &zi) in z.iter().enumerate() {
                    println!("value of z {:?} for cycle {:?}", solution.value(zi), i);
                    if solution.value(zi) > 0.5 {
                        let input_amount = solution.value(x[i]);
                        let mut current_amount = input_amount;

                        println!("\nAnalyzing path {}:", i);
                        println!("Input amount: {}", input_amount);

                        println!("x vaue for {:?} is {:?}", i, current_amount);

                        // Calculate actual output through the path
                        for pool in &cycle_pools[i] {
                            let reserve0 = pool.reserve0.as_u128() as f64;
                            let reserve1 = pool.reserve1.as_u128() as f64;
                            let fee = 1.0 - (pool.router_fee.as_u64() as f64 / 10000.0);

                            // Apply constant product formula
                            current_amount = (reserve1 * current_amount * fee)
                                / (reserve0 + current_amount * fee);

                            println!("After pool {}: {}", pool.address, current_amount);
                        }

                        let profit = current_amount - input_amount;
                        if profit > 0.0 {
                            println!("jnbchcsbhb");
                            println!("\nFound profitable path!");
                            println!("Path length: {}", cycle_pools[i].len());
                            println!("Input: {}", input_amount);
                            println!("Output: {}", current_amount);
                            println!("Profit: {}", profit);
                            println!("Return: {:.2}%", (profit / input_amount) * 100.0);

                            profitable_paths.push((cycle_pools[i].clone(), input_amount, profit));
                        }
                    }
                }
            }
            Err(e) => println!("Failed to solve optimization problem: {:?}", e),
        }

        // Sort profitable paths by profit
        profitable_paths.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        println!("nnnnnnnnnn {:?}", profitable_paths);
        profitable_paths
    }

    pub fn print_profitable_paths(
        profitable_paths: &[(Vec<UniV2Pool>, f64, f64)],
        mut starting_token: Address,
    ) {
        for (i, (path, input, profit)) in profitable_paths.iter().enumerate() {
            println!("\nProfitable Path {}:", i + 1);
            println!("Input Amount: {}", input);
            println!("Expected Profit: {}", profit);
            println!("Return: {:.2}%", (profit / input) * 100.0);
            println!("Path:");

            let mut current_token = starting_token;

            for (j, pool) in path.iter().enumerate() {
                // Ensure the path follows the token flow
                if pool.token0 == starting_token {
                    current_token = pool.token1;
                } else if pool.token1 == current_token {
                    current_token = pool.token0;
                } else {
                    println!("  Error: Invalid token flow in path.");
                    break;
                }

                println!("  Step {}: {} -> {}", j + 1, starting_token, current_token);
                println!("    Pool: {}", pool.address);
                println!("    Reserves: {} / {}", pool.reserve0, pool.reserve1);
                println!("    Fee: {} bps", pool.router_fee);
                starting_token = current_token;
            }
        }
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
            reserve0: U256::from(1000),
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
            reserve0: U256::from(1200),
            reserve1: U256::from(3000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x19; 20]),
            token0: tokens[4],
            token1: tokens[1], // Extra interlinking
            reserve0: U256::from(3000),
            reserve1: U256::from(1000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
    ];

    print!("sjvncjsnfvc");

    // Create the PoolGraph from the pool instances
    let pool_graph = PoolGraph::new(&pools);
    let cycles: Vec<Vec<ethers::types::H160>> = pool_graph.detect_cycles(tokens[4]);
    let mut cycle_pools: Vec<Vec<UniV2Pool>> = Vec::new();

    for (i, cycle) in cycles.iter().enumerate() {
        println!("\nCycle {}:", i + 1);
        pool_graph.print_cycle_details(cycle);

        if let Some(pool_path) = pool_graph.convert_cycle_to_pools(cycle) {
            cycle_pools.push(pool_path.clone());
        }
    }
    println!("cycle 8 is ");

    pool_graph.print_cycle_details(&cycles[8]);
    println!("Found {} potential cycles", cycle_pools.len());
    let profitable_paths = pool_graph.find_arb(cycle_pools);

    println!("cycle which give profit is {:?}", cycles[8]);

    if profitable_paths.is_empty() {
        println!("No profitable arbitrage opportunities found");
    } else {
        println!("\nFound {} profitable paths", profitable_paths.len());

        println!("profitable path {:?}", profitable_paths);

        PoolGraph::print_profitable_paths(&profitable_paths, tokens[4]);
    }
}
