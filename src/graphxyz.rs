use std::collections::HashMap;
use totsu::prelude::*;
use totsu::totsu_core::ConeZero;
use totsu::totsu_core::MatOp;
use totsu::*;
use totsu_core::solver::Operator;

type La = FloatGeneric<f64>;

const NUM_POOLS: usize = 8;
const NUM_ASSETS: usize = 5;

#[derive(Debug)]
struct ArbitragePath {
    pools: Vec<(usize, usize)>,
    amounts: Vec<f64>,
    profit: f64,
}

#[derive(Debug, Clone)]
struct Pool {
    index: usize,
    reserve0: f64,
    reserve1: f64,
    fee: f64,
}

impl Pool {
    fn get_spot_price(&self, zero_for_one: bool) -> f64 {
        if zero_for_one {
            self.reserve1 / self.reserve0
        } else {
            self.reserve0 / self.reserve1
        }
    }
}

fn create_sample_pools() -> HashMap<(usize, usize), Pool> {
    let mut graph = HashMap::new();

    let pool_configs = [
        ((0, 1), (1000.0, 2050.0)), // ETH-USDC
        ((1, 2), (2000.0, 1000.0)), // USDC-DAI
        ((2, 0), (1100.0, 500.0)),  // DAI-ETH
        ((2, 3), (1500.0, 1480.0)),
        ((3, 4), (1200.0, 1000.0)),
        ((4, 0), (1800.0, 900.0)),
        ((1, 3), (1300.0, 1700.0)),
        ((2, 4), (1900.0, 1100.0)),
    ];

    for (i, ((from, to), (reserve0, reserve1))) in pool_configs.iter().enumerate() {
        graph.insert(
            (*from, *to),
            Pool {
                index: i,
                reserve0: *reserve0,
                reserve1: *reserve1,
                fee: 0.003,
            },
        );
    }
    graph
}

fn prepare_lp_data(
    graph: &HashMap<(usize, usize), Pool>,
    start_asset: usize,
) -> (MatBuild<La>, MatBuild<La>, MatBuild<La>, Vec<f64>) {
    let n_vars = graph.len();
    let n_constraints = NUM_ASSETS;

    println!("Creating matrices with dimensions:");
    println!("Variables: {}", n_vars);
    println!("Constraints: {}", n_constraints);

    let mut mat_g = MatBuild::new(MatType::General(n_constraints, n_vars));
    let mut vec_h = MatBuild::new(MatType::General(n_constraints, 1));
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));

    // Initialize to zero
    for i in 0..n_constraints {
        for j in 0..n_vars {
            mat_g[(i, j)] = 0.0;
        }
        vec_h[(i, 0)] = 0.0;
    }

    for j in 0..n_vars {
        vec_c[(j, 0)] = 0.0;
    }

    // Set up flow conservation constraints
    for ((from, to), pool) in graph.iter() {
        mat_g[(*from, pool.index)] = -1.0;
        let effective_price = pool.get_spot_price(true) * (1.0 - pool.fee);
        mat_g[(*to, pool.index)] = effective_price;
        vec_c[(pool.index, 0)] = -effective_price;
    }

    // All net flows must sum to zero
    for i in 0..n_constraints {
        vec_h[(i, 0)] = 0.0;
    }

    // Calculate a much larger workspace size
    // The solver needs space for:
    // - primal variables (n_vars)
    // - dual variables (n_constraints)
    // - temporary matrices and vectors
    // - internal solver state
    let m = n_constraints;
    let n = n_vars;

    // Conservative estimate of required workspace
    let required_len = 2 * (m + n) +     // For primal and dual variables
                      2 * m * n +         // For matrix operations
                      3 * m +             // For residuals
                      3 * n +             // For search directions
                      n * m +             // For the Hessian
                      100; // Extra buffer space

    println!("Required workspace length: {}", required_len);
    let workspace = vec![0.0; required_len];

    // Print matrix dimensions for debugging
    println!("Matrix G dimensions: {}x{}", mat_g.size().0, mat_g.size().1);
    println!("Vector h dimensions: {}x{}", vec_h.size().0, vec_h.size().1);
    println!("Vector c dimensions: {}x{}", vec_c.size().0, vec_c.size().1);

    // Print first few elements of each matrix for sanity check
    println!("\nFirst few elements of matrices:");
    println!("G[0,0] = {}", mat_g[(0, 0)]);
    println!("h[0] = {}", vec_h[(0, 0)]);
    println!("c[0] = {}", vec_c[(0, 0)]);

    (vec_c, mat_g, vec_h, workspace)
}

fn find_arbitrage(
    graph: &HashMap<(usize, usize), Pool>,
    start_asset: usize,
) -> Result<Option<ArbitragePath>, Box<dyn std::error::Error>> {
    let (mut vec_c, mut mat_g, mut vec_h, mut work) = prepare_lp_data(graph, start_asset);

    let n_vars = graph.len();

    let cost_coeffs: Vec<f64> = (0..n_vars).map(|i| vec_c[(i, 0)]).collect();

    let vec_c_op: MatOp<La> = vec_c.as_op();
    let mat_g_op: MatOp<La> = mat_g.as_op();
    let vec_h_op: MatOp<La> = vec_h.as_op();

    // ConeZero enforces that Gx + h = 0
    let cone = ConeZero::<La>::new();

    let mut solver = Solver::<La>::new().par(|p| {
        p.eps_acc = 1e-9;
        p.max_iter = Some(1000);
        p.log_period = 100;
    });

    println!("Starting solver...");
    match solver.solve((vec_c_op, mat_g_op, vec_h_op, cone, &mut work)) {
        Ok(result) => {
            let (x, y) = result;

            if x.len() != n_vars {
                println!(
                    "Solution vector length mismatch. Expected: {}, Got: {}",
                    n_vars,
                    x.len()
                );
                return Ok(None);
            }

            let profit: f64 = -x
                .iter()
                .zip(cost_coeffs.iter())
                .map(|(&xi, &ci)| xi * ci)
                .sum::<f64>();

            if profit > 1e-6 {
                let mut active_pools = Vec::new();
                let mut active_amounts = Vec::new();

                for (i, &amount) in x.iter().enumerate() {
                    if amount > 1e-6 {
                        if let Some(((from, to), _)) = graph.iter().find(|(_, p)| p.index == i) {
                            active_pools.push((*from, *to));
                            active_amounts.push(amount);
                        }
                    }
                }

                if !active_pools.is_empty() {
                    Ok(Some(ArbitragePath {
                        pools: active_pools,
                        amounts: active_amounts,
                        profit,
                    }))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            println!("Solver error: {:?}", e);
            Err(Box::new(e))
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = create_sample_pools();

    println!("Number of pools: {}", graph.len());

    for start_asset in 0..NUM_ASSETS {
        println!(
            "\nSearching for arbitrage starting from asset {}",
            start_asset
        );

        match find_arbitrage(&graph, start_asset)? {
            Some(arb) => {
                println!("Found profitable arbitrage:");
                println!("Path: {:?}", arb.pools);
                println!("Amounts: {:?}", arb.amounts);
                println!("Expected profit: {:.6}", arb.profit);
            }
            None => println!(
                "No profitable arbitrage found starting from asset {}",
                start_asset
            ),
        }
    }

    Ok(())
}
