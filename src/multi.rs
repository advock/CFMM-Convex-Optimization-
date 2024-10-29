use totsu::prelude::*;
use totsu::{MatBuild, ProbLP, ProbSOCP};
type La = FloatGeneric<f64>;

fn main() {
    let num_assets = 5; // Five assets
    let num_pools = 8; // Eight pools
    let reserves: Vec<Vec<f64>> = vec![
        vec![100.0, 10.0],
        vec![90.0, 15.0],
        vec![80.0, 8.0],
        vec![70.0, 12.0],
        vec![60.0, 14.0],
        vec![110.0, 20.0],
        vec![130.0, 25.0],
        vec![120.0, 18.0],
    ];
    let fee: f64 = 0.997;
    let n_vars = num_pools * 2; // Two variables per pool (buy/sell)

    // Objective vector `c` for profit maximization
    let mut vec_c = MatBuild::<FloatGeneric<f64>>::new(MatType::General(n_vars, 1));
    for i in 0..n_vars {
        vec_c[(i, 0)] = if i % 2 == 0 { -1.0 } else { 1.0 };
    }

    // Calculate total constraints count
    let num_constraints = 2 * num_pools + num_assets + n_vars; // Separate out each constraint type
    let mut mat_g = MatBuild::<FloatGeneric<f64>>::new(MatType::General(num_constraints, n_vars));
    let mut vec_h = MatBuild::<FloatGeneric<f64>>::new(MatType::General(num_constraints, 1));

    // Constant product constraints for each pool
    for (i, reserve) in reserves.iter().enumerate() {
        mat_g[(i * 2, i * 2)] = reserve[1];
        mat_g[(i * 2, i * 2 + 1)] = -reserve[0] / fee;
        vec_h[(i * 2, 0)] = reserve[0] * reserve[1] * 0.01;

        mat_g[(i * 2 + 1, i * 2)] = -1.0;
        mat_g[(i * 2 + 1, i * 2 + 1)] = 1.0;
        vec_h[(i * 2 + 1, 0)] = 0.0;
    }

    // Balance constraints across assets
    for asset_idx in 0..num_assets {
        for pool_idx in 0..num_pools {
            let asset_in_pool = (asset_idx + pool_idx) % num_assets;
            let row = 2 * num_pools + asset_idx;
            if pool_idx * 2 < n_vars {
                mat_g[(row, pool_idx * 2)] = if asset_in_pool == asset_idx {
                    1.0
                } else {
                    -1.0
                };
                mat_g[(row, pool_idx * 2 + 1)] = if asset_in_pool == asset_idx {
                    -1.0
                } else {
                    1.0
                };
            }
            vec_h[(row, 0)] = 0.0;
        }
    }

    // Non-negativity constraints
    for i in 0..n_vars {
        let row = 2 * num_pools + num_assets + i;
        mat_g[(row, i)] = -1.0;
        vec_h[(row, 0)] = 0.0;
    }

    let mat_a = MatBuild::<FloatGeneric<f64>>::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::<FloatGeneric<f64>>::new(MatType::General(0, 1));

    let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);
    let mut solver = Solver::new();

    solver = solver.par(|p| {
        p.eps_acc = 1e-12;
        p.eps_inf = 1e-12;
        p.eps_zero = 1e-14;
        p.max_iter = Some(1000000);
        p.log_period = 10000;
    });

    let (op_c, op_a, op_b, cone, mut work) = prob.problem();
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
        Ok((x, _y)) => {
            println!("Optimal solution found:");
            for i in 0..num_pools {
                println!(
                    "Pool {} - Trade Asset-A: {:.12}, Receive Asset-B: {:.12}",
                    i + 1,
                    x[i * 2],
                    x[i * 2 + 1]
                );
            }
            let profit: f64 =
                x.iter().step_by(2).sum::<f64>() - x.iter().skip(1).step_by(2).sum::<f64>();
            println!("Solution vector: {:?}", x);
            println!("Profit: {:.16}", profit);

            for (i, reserve) in reserves.iter().enumerate() {
                let new_reserve_a = reserve[0] + x[i * 2];
                let new_reserve_b = reserve[1] - x[i * 2 + 1] / fee;
                let cp_satisfied =
                    (new_reserve_a * new_reserve_b >= reserve[0] * reserve[1] * 0.9999);
                println!(
                    "Pool {} constant product formula satisfied: {}",
                    i + 1,
                    cp_satisfied
                );
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
            println!("Objective vector c:");
            println!("{:?}", vec_c);
            println!("Constraint matrix G:");
            println!("{:?}", mat_g);
            println!("Constraint vector h:");
            println!("{:?}", vec_h);
        }
    }
}
