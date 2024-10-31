use ethers::types::Address;
use ethers::types::U256;
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
            self.dfs_cycle_detection(start_idx, &mut visited, &mut stack, &mut cycles);
        }

        // Convert node indices back to Addresses for output
        cycles
            .iter()
            .map(|cycle| cycle.iter().map(|&idx| self.graph[idx].clone()).collect())
            .collect()
    }

    fn dfs_cycle_detection(
        &self,
        node: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        stack: &mut Vec<NodeIndex>,
        cycles: &mut Vec<Vec<NodeIndex>>,
    ) {
        if !visited.insert(node) {
            // If we revisit a node in the current path, we have a cycle
            if let Some(pos) = stack.iter().position(|&n| n == node) {
                let cycle = stack[pos..].to_vec();
                cycles.push(cycle);
            }
            return;
        }

        stack.push(node);

        for edge in self.graph.edges(node) {
            let neighbor = edge.target();
            if !visited.contains(&neighbor) || stack.contains(&neighbor) {
                self.dfs_cycle_detection(neighbor, visited, stack, cycles);
            }
        }

        stack.pop();
        visited.remove(&node);
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

fn main() {
    // Create instances of UniV2Pool
    let pools = vec![
        UniV2Pool {
            address: Address::from([0x1; 20]),
            token0: Address::from([0x0; 20]),
            token1: Address::from([0x1; 20]),
            reserve0: U256::from(1000),
            reserve1: U256::from(2000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x2; 20]),
            token0: Address::from([0x1; 20]),
            token1: Address::from([0x2; 20]),
            reserve0: U256::from(1500),
            reserve1: U256::from(2500),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
        UniV2Pool {
            address: Address::from([0x3; 20]),
            token0: Address::from([0x2; 20]),
            token1: Address::from([0x0; 20]),
            reserve0: U256::from(3000),
            reserve1: U256::from(1000),
            router_fee: U256::from(30),
            fees0: U256::from(5),
            fees1: U256::from(5),
        },
    ];

    // Create the PoolGraph from the pool instances
    let pool_graph = PoolGraph::new(&pools);
    pool_graph.export_visualization("only_usdc4.dot").unwrap();
}
