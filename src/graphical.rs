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
        ((0, 1), (1000.0, 2050.0)), // More diverse pricing
        ((1, 2), (2000.0, 1950.0)),
        ((2, 0), (1100.0, 1050.0)),
        ((2, 3), (1500.0, 1480.0)),
        ((3, 4), (1200.0, 1180.0)),
        ((4, 0), (1800.0, 1750.0)),
        ((1, 3), (1300.0, 1280.0)),
        ((2, 4), (1900.0, 1850.0)),
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
) -> (MatBuild<La>, MatBuild<La>, MatBuild<La>) {
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
        vec_c[(j, 0)] -= 1e-2;
    }

    // Set up flow conservation constraints
    for ((from, to), pool) in graph.iter() {
        mat_g[(*from, pool.index)] = -1.0;
        let effective_price = pool.get_spot_price(true) * (1.0 - pool.fee);
        mat_g[(*to, pool.index)] = effective_price;
        vec_c[(pool.index, 0)] = -effective_price;
    }

    // Print the entire constraint matrix G
    println!("\nConstraint Matrix G:");
    for i in 0..n_constraints {
        print!("Row {}: ", i);
        for j in 0..n_vars {
            print!("{:8.4} ", mat_g[(i, j)]);
        }
        println!();
    }

    // Print vector h
    println!("\nVector h:");
    for i in 0..n_constraints {
        println!("h[{}] = {:.4}", i, vec_h[(i, 0)]);
    }

    // Print cost vector c
    println!("\nCost vector c:");
    for i in 0..n_vars {
        println!("c[{}] = {:.4}", i, vec_c[(i, 0)]);
    }

    // Print pool information
    println!("\nPool Information:");
    for ((from, to), pool) in graph.iter() {
        println!(
            "Pool {}: {} -> {}, Price = {:.4}, Fee-adjusted = {:.4}",
            pool.index,
            from,
            to,
            pool.get_spot_price(true),
            pool.get_spot_price(true) * (1.0 - pool.fee)
        );
    }

    // Verify constraint matrix properties
    let mut row_sums = vec![0.0; n_constraints];
    for i in 0..n_constraints {
        for j in 0..n_vars {
            let val: f64 = mat_g[(i, j)];
            row_sums[i] += val.abs();
        }
    }
    println!("\nRow sums (should be non-zero for each asset):");
    for (i, sum) in row_sums.iter().enumerate() {
        println!("Asset {}: {:.4}", i, sum);
    }

    let m = n_constraints;
    let n = n_vars;

    // Use a larger workspace size
    let required_len = 2 * (m + n) +     // Variables
                      2 * m * n +         // Matrix operations
                      3 * m +             // Residuals
                      3 * n +             // Search directions
                      n * m +             // Hessian
                      m * m +             // Additional matrix ops
                      n * n +             // More matrix ops
                      1000; // Extra buffer

    println!("\nRequired workspace length: {}", required_len);
    let workspace = vec![0.0; required_len];

    println!("Final matrix dimensions:");
    println!("G: {}x{}", mat_g.size().0, mat_g.size().1);
    println!("h: {}x{}", vec_h.size().0, vec_h.size().1);
    println!("c: {}x{}", vec_c.size().0, vec_c.size().1);

    (vec_c, mat_g, vec_h)
}

fn find_arbitrage(
    graph: &HashMap<(usize, usize), Pool>,
    start_asset: usize,
) -> Result<Option<ArbitragePath>, Box<dyn std::error::Error>> {
    let (mut vec_c, mut mat_g, mut vec_h) = prepare_lp_data(graph, start_asset);

    let n_vars = graph.len();

    let cost_coeffs: Vec<f64> = (0..n_vars).map(|i| vec_c[(i, 0)]).collect();

    // let vec_c_op: MatOp<La> = vec_c.as_op();
    // let mat_g_op: MatOp<La> = mat_g.as_op();
    // let vec_h_op: MatOp<La> = vec_h.as_op();

    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);

    let cone = ConeZero::<La>::new();

    let mut solver = Solver::<La>::new().par(|p| {
        p.eps_acc = 1e-6; // Relax from 1e-9
        p.eps_inf = 1e-6; // Add infeasibility tolerance
        p.max_iter = Some(500000); // Increase iterations
        p.log_period = 1000;
    });

    let (op_c, op_a, op_b, cone, mut work) = prob.problem();

    println!("\nStarting solver...");
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
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
            // ... (rest of the function remains the same)
        }
        Err(e) => {
            println!("Solver error: {:?}", e);
            println!("This might indicate:");
            println!("1. Infeasible problem");
            println!("2. Numerical issues");
            println!("3. Insufficient workspace");
            println!("4. Incorrect problem formulation");
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
