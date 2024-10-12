use totsu::prelude::*;
use totsu::MatBuild;
use totsu::ProbLP;
use totsu::*;
use totsu_core::MatOp;
use totsu_core::{ConeRPos, ConeZero};

type La = FloatGeneric<f64>;
//type ASolver = Solver<La>;

fn main() {
    const N_TOKENS: usize = 4; // Number of tokens (n)
    const N_CFMMS: usize = 5; // Number of CFMMs (m)

    let reserves = vec![
        vec![40.0, 40.0, 40.0, 40.0], // Balancer pool
        vec![100.0, 10.0],            // UniswapV2 pool: TOKEN-0/TOKEN-1
        vec![10.0, 50.0],             // UniswapV2 pool: TOKEN-1/TOKEN-2
        vec![400.0, 500.0],           // UniswapV2 pool: TOKEN-2/TOKEN-3
        vec![100.0, 100.0],           // Constant Sum pool: TOKEN-2/TOKEN-3
    ];

    let fees = vec![0.997, 0.997, 0.997, 0.997, 0.999]; // Pool fees (slightly reduced)
    let market_value = vec![1.0, 10.0, 2.0, 3.0]; // Market values for tokens

    // Set up the Totsu solver
    //sjm
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-8;
        p.eps_inf = 1e-8;
        p.max_iter = Some(100000);
    });

    // Objective vector `c`
    let n_vars = 2 * N_TOKENS * N_CFMMS;
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
    vec_c.set_by_fn(|r, _| {
        let cfmm = r / (2 * N_TOKENS);
        let token = (r % (2 * N_TOKENS)) / 2;
        if r % 2 == 0 {
            market_value[token] // For lambda
        } else {
            -market_value[token] // For delta
        }
    });

    // Create constraint matrices G, h for all constraints
    let mut total_rows = 0;
    for i in 0..N_CFMMS {
        total_rows += 1 + reserves[i].len(); // Pool-specific constraint + new_reserves >= 0 constraints
    }

    let mut mat_g = MatBuild::new(MatType::General(total_rows, n_vars));
    let mut vec_h = MatBuild::new(MatType::General(total_rows, 1));

    let mut row = 0;

    // Add pool constraints
    for i in 0..N_CFMMS {
        let pool_reserves = &reserves[i];
        let n_pool_tokens = pool_reserves.len();

        // Inequality constraint: new_reserves >= 0
        for j in 0..n_pool_tokens {
            mat_g.set_by_fn(|r, c| {
                if r == row && c == 2 * (i * N_TOKENS + j) {
                    -1.0 // -lambda
                } else if r == row && c == 2 * (i * N_TOKENS + j) + 1 {
                    fees[i] // fee * delta
                } else {
                    0.0
                }
            });
            vec_h[(row, 0)] = -pool_reserves[j]; // -original_reserve
            row += 1;
        }

        // Pool-specific constraints
        match i {
            0 => {
                // Balancer Pool
                let geo_mean_original = pool_reserves
                    .iter()
                    .product::<f64>()
                    .powf(1.0 / n_pool_tokens as f64);
                mat_g.set_by_fn(|r, c| {
                    if r == row && c / 2 / N_TOKENS == i {
                        let j = (c / 2) % N_TOKENS;
                        if j < n_pool_tokens {
                            if c % 2 == 0 {
                                -1.0 / n_pool_tokens as f64 // -lambda
                            } else {
                                fees[i] / n_pool_tokens as f64 // fee * delta
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                });
                vec_h[(row, 0)] = -geo_mean_original * 0.999; // Slight relaxation
                row += 1;
            }
            1..=3 => {
                // UniswapV2 Pools
                let product_original = pool_reserves[0] * pool_reserves[1];
                mat_g.set_by_fn(|r, c| {
                    if r == row && c / 2 / N_TOKENS == i {
                        let j = (c / 2) % N_TOKENS;
                        if j < 2 {
                            if c % 2 == 0 {
                                -pool_reserves[1 - j] // -lambda * other_reserve
                            } else {
                                fees[i] * pool_reserves[1 - j] // fee * delta * other_reserve
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                });
                vec_h[(row, 0)] = -product_original * 0.999; // Slight relaxation
                row += 1;
            }
            4 => {
                // Constant Sum Pool
                let sum_original = pool_reserves.iter().sum::<f64>();
                mat_g.set_by_fn(|r, c| {
                    if r == row && c / 2 / N_TOKENS == i {
                        let j = (c / 2) % N_TOKENS;
                        if j < n_pool_tokens {
                            if c % 2 == 0 {
                                -1.0 // -lambda
                            } else {
                                fees[i] // fee * delta
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                });
                vec_h[(row, 0)] = -sum_original * 0.999; // Slight relaxation
                row += 1;
            }
            _ => unreachable!(),
        }
    }

    // Create the cone
    let cone = ConeRPos::<La>::new();

    // Prepare work memory
    let work_len = Solver::<La>::query_worklen((total_rows, n_vars));
    let mut work = vec![0.0; work_len];

    // Create operators
    let op_c = vec_c.as_op();
    let op_g = mat_g.as_op();
    let op_h = vec_h.as_op();

    // Solve the problem
    match solver.solve((op_c, op_g, op_h, cone, &mut work)) {
        Ok((x, y)) => {
            println!("Optimal solution found:");
            for i in 0..N_CFMMS {
                for j in 0..N_TOKENS {
                    let lambda = x[2 * (i * N_TOKENS + j)];
                    let delta = x[2 * (i * N_TOKENS + j) + 1];
                    if lambda.abs() > 1e-6 || delta.abs() > 1e-6 {
                        println!(
                            "CFMM {}, Token {}: lambda = {}, delta = {}",
                            i, j, lambda, delta
                        );
                    }
                }
            }

            // Calculate objective value
            let mut obj_val = 0.0;
            for i in 0..n_vars {
                obj_val += vec_c[(i, 0)] * x[i];
            }
            println!("Objective value: {}", obj_val);
        }
        Err(e) => println!("Error: {:?}", e),
    }
}
