use env_logger;
use log::{self, log_enabled};
use totsu::prelude::*;
use totsu::{MatBuild, ProbSOCP};

type La = FloatGeneric<f64>;

fn main() {
    const N_TOKENS: usize = 4;
    const N_CFMMS: usize = 5;

    let reserves: Vec<Vec<f64>> = vec![
        vec![4.0, 4.0, 4.0, 4.0], // Balancer pool
        vec![10.0, 1.0],          // UniswapV2 pool: TOKEN-0/TOKEN-1
        vec![1.0, 5.0],           // UniswapV2 pool: TOKEN-1/TOKEN-2
        vec![40.0, 50.0],         // UniswapV2 pool: TOKEN-2/TOKEN-3
        vec![10.0, 10.0],         // Constant Sum pool: TOKEN-2/TOKEN-3
    ];

    let fees: Vec<f64> = vec![0.998, 0.997, 0.997, 0.997, 0.999];
    let market_value: Vec<f64> = vec![1.0, 12.0, 1.8, 3.5];

    // Create variables
    let n_vars = 2 * N_TOKENS * N_CFMMS;

    // Objective vector `f`
    let mut vec_f = MatBuild::new(MatType::General(n_vars, 1));
    vec_f.set_by_fn(|r, _| {
        let token = (r % (2 * N_TOKENS)) / 2;
        if r % 2 == 0 {
            -market_value[token] // For lambda (negative because we're minimizing)
        } else {
            market_value[token] // For delta
        }
    });

    // Create SOCP constraint matrices
    let mut mats_g = Vec::new();
    let mut vecs_h = Vec::new();
    let mut vecs_c = Vec::new();
    let mut scls_d = Vec::new();

    for i in 0..N_CFMMS {
        let pool_reserves = &reserves[i];
        let n_pool_tokens = pool_reserves.len();

        match i {
            0 => {
                // Balancer Pool (geometric mean)
                let mut g = MatBuild::new(MatType::General(n_pool_tokens + 1, n_vars));
                let mut h = MatBuild::new(MatType::General(n_pool_tokens + 1, 1));
                let mut c = MatBuild::new(MatType::General(n_vars, 1));

                for j in 0..n_pool_tokens {
                    let lambda_idx = 2 * (i * N_TOKENS + j);
                    let delta_idx = lambda_idx + 1;

                    g[(j, lambda_idx)] = -1.0 / (pool_reserves[j] as f64).sqrt();
                    g[(j, delta_idx)] = fees[i] / (pool_reserves[j] as f64).sqrt();
                    h[(j, 0)] = (pool_reserves[j] as f64).sqrt();
                    g[(n_pool_tokens, lambda_idx)] = -1.0;
                    g[(n_pool_tokens, delta_idx)] = fees[i];
                }
                h[(n_pool_tokens, 0)] = 0.0; // Allow for slight imbalance

                mats_g.push(g);
                vecs_h.push(h);
                vecs_c.push(c);
                scls_d.push(0.0);
            }
            1..=3 => {
                // UniswapV2 Pools (constant product)
                let mut g = MatBuild::new(MatType::General(3, n_vars));
                let mut h = MatBuild::new(MatType::General(3, 1));
                let mut c = MatBuild::new(MatType::General(n_vars, 1));

                let lambda_idx_0 = 2 * (i * N_TOKENS);
                let delta_idx_0 = lambda_idx_0 + 1;
                let lambda_idx_1 = lambda_idx_0 + 2;
                let delta_idx_1 = lambda_idx_1 + 1;

                g[(0, lambda_idx_0)] = -1.0;
                g[(0, delta_idx_0)] = fees[i];
                g[(1, lambda_idx_1)] = -1.0;
                g[(1, delta_idx_1)] = fees[i];
                g[(2, lambda_idx_0)] = -0.5 / (pool_reserves[0] as f64).sqrt();
                g[(2, delta_idx_0)] = 0.5 * fees[i] / (pool_reserves[0] as f64).sqrt();
                g[(2, lambda_idx_1)] = -0.5 / (pool_reserves[1] as f64).sqrt();
                g[(2, delta_idx_1)] = 0.5 * fees[i] / (pool_reserves[1] as f64).sqrt();

                h[(0, 0)] = pool_reserves[0];
                h[(1, 0)] = pool_reserves[1];
                h[(2, 0)] = (pool_reserves[0] * pool_reserves[1] as f64).sqrt() * 0.999; // Allow for slight imbalance

                mats_g.push(g);
                vecs_h.push(h);
                vecs_c.push(c);
                scls_d.push(0.0);
            }
            4 => {
                // Constant Sum Pool (linear constraint)
                let mut g = MatBuild::new(MatType::General(1, n_vars));
                let mut h = MatBuild::new(MatType::General(1, 1));
                let mut c = MatBuild::new(MatType::General(n_vars, 1));

                for j in 0..n_pool_tokens {
                    let lambda_idx = 2 * (i * N_TOKENS + j);
                    let delta_idx = lambda_idx + 1;

                    g[(0, lambda_idx)] = -1.0;
                    g[(0, delta_idx)] = fees[i];
                }

                h[(0, 0)] = pool_reserves.iter().sum::<f64>() * 0.999; // Allow for slight imbalance

                mats_g.push(g);
                vecs_h.push(h);
                vecs_c.push(c);
                scls_d.push(0.0);
            }
            _ => unreachable!(),
        }
    }

    // Add non-negativity constraints for all variables
    let mut g_nonneg = MatBuild::new(MatType::General(n_vars, n_vars));
    let mut h_nonneg = MatBuild::new(MatType::General(n_vars, 1));
    for i in 0..n_vars {
        g_nonneg[(i, i)] = 1.0;
        h_nonneg[(i, 0)] = 0.0;
    }
    mats_g.push(g_nonneg);
    vecs_h.push(h_nonneg);
    vecs_c.push(MatBuild::new(MatType::General(n_vars, 1)));
    scls_d.push(0.0);

    // Add a constraint to limit the total amount traded
    let mut g_limit = MatBuild::new(MatType::General(1, n_vars));
    let mut h_limit = MatBuild::new(MatType::General(1, 1));
    for i in 0..n_vars {
        g_limit[(0, i)] = 1.0;
    }
    h_limit[(0, 0)] = reserves.iter().flatten().sum::<f64>() * 0.9; // Increase limit to 75% of total reserves
    mats_g.push(g_limit);
    vecs_h.push(h_limit);
    vecs_c.push(MatBuild::new(MatType::General(n_vars, 1)));
    scls_d.push(0.0);

    // Create empty matrices for equality constraints (we don't have any in this problem)
    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    // Create the ProbSOCP struct
    let mut prob = ProbSOCP::new(vec_f.clone(), mats_g, vecs_h, vecs_c, scls_d, mat_a, vec_b);

    // Set up the Totsu solver
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-5;
        p.eps_inf = 1e-5;
        p.max_iter = Some(1000000);
    });

    // Generate the problem components and solve
    let (op_c, op_a, op_b, cone, mut work) = prob.problem();
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
        Ok((x, _y)) => {
            println!("Optimal solution found:");
            for i in 0..N_CFMMS {
                for j in 0..N_TOKENS {
                    let lambda = x[2 * (i * N_TOKENS + j)];
                    let delta = x[2 * (i * N_TOKENS + j) + 1];
                    if lambda.abs() > 1e-6 || delta.abs() > 1e-6 {
                        println!(
                            "CFMM {}, Token {}: lambda = {:.6}, delta = {:.6}",
                            i, j, lambda, delta
                        );
                    }
                }
            }

            // Calculate and print the objective value
            let obj_val: f64 = x
                .iter()
                .enumerate()
                .map(|(i, &val)| -vec_f[(i, 0)] * val)
                .sum();
            println!("Objective value: {:.6}", obj_val);
        }
        Err(e) => println!("Error: {:?}", e),
    }
}
