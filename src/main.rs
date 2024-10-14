use totsu::prelude::*;
use totsu::{MatBuild, ProbLP, ProbSOCP};
type La = FloatGeneric<f64>;

fn main() {
    // Two pools with different prices
    let reserves1: Vec<f64> = vec![100.0, 10.0]; // Pool 1: TOKEN-0/TOKEN-1
    let reserves2: Vec<f64> = vec![90.0, 12.0]; // Pool 2: TOKEN-0/TOKEN-1
    let fee: f64 = 0.997; // 0.3% fee

    // Create variables: [x1, y1, x2, y2]
    // x1: amount of TOKEN-0 to trade in Pool 1
    // y1: amount of TOKEN-1 to receive from Pool 1
    // x2: amount of TOKEN-1 to trade in Pool 2
    // y2: amount of TOKEN-0 to receive from Pool 2
    let n_vars = 4;

    // Objective vector `c` (maximize profit)
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
    vec_c.set_by_fn(|r, _| {
        match r {
            1 | 3 => 1.0,  // Receive
            0 | 2 => -1.0, // Trade
            _ => 0.0,      // Default case (shouldn't occur in our 4-variable setup)
        }
    });

    // Constraint matrix G and vector h for Gx <= h
    let mut mat_g = MatBuild::new(MatType::General(10, n_vars));
    let mut vec_h = MatBuild::new(MatType::General(10, 1));

    // Constant product constraints
    mat_g[(0, 0)] = reserves1[1];
    mat_g[(0, 1)] = -reserves1[0] * fee;
    vec_h[(0, 0)] = 0.0; // x1 * reserves1[1] <= y1 * reserves1[0] * fee
    mat_g[(1, 2)] = reserves2[0];
    mat_g[(1, 3)] = -reserves2[1] * fee;
    vec_h[(1, 0)] = 0.0; // x2 * reserves2[0] <= y2 * reserves2[1] * fee

    // Arbitrage constraints
    mat_g[(2, 1)] = 1.0;
    mat_g[(2, 2)] = -1.0;
    vec_h[(2, 0)] = 0.0; // y1 <= x2
    mat_g[(3, 3)] = 1.0;
    mat_g[(3, 0)] = -1.0;
    vec_h[(3, 0)] = 0.0; // y2 <= x1

    // Non-negativity constraints
    mat_g[(4, 0)] = -1.0;
    vec_h[(4, 0)] = 0.0; // -x1 <= 0
    mat_g[(5, 1)] = -1.0;
    vec_h[(5, 0)] = 0.0; // -y1 <= 0
    mat_g[(6, 2)] = -1.0;
    vec_h[(6, 0)] = 0.0; // -x2 <= 0
    mat_g[(7, 3)] = -1.0;
    vec_h[(7, 0)] = 0.0; // -y2 <= 0

    // Upper bound constraints (limit trades to 50% of pool reserves)
    mat_g[(8, 0)] = 1.0;
    vec_h[(8, 0)] = reserves1[0] * 0.5; // x1 <= 50% of reserves1[0]
    mat_g[(9, 2)] = 1.0;
    vec_h[(9, 0)] = reserves2[1] * 0.5; // x2 <= 50% of reserves2[1]

    // Create empty matrices for equality constraints (we don't have any in this problem)
    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    // Create the ProbLP struct
    let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);

    // Set up the Totsu solver
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-8;
        p.eps_inf = 1e-8;
        p.eps_zero = 1e-10;
        p.max_iter = Some(1000000);
        p.log_period = 10000;
    });

    // Generate the problem components and solve
    let (op_c, op_a, op_b, cone, mut work) = prob.problem();
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
        Ok((x, _y)) => {
            println!("Optimal solution found:");
            println!(
                "Pool 1 - Trade TOKEN-0: {:.6}, Receive TOKEN-1: {:.6}",
                x[0], x[1]
            );
            println!(
                "Pool 2 - Trade TOKEN-1: {:.6}, Receive TOKEN-0: {:.6}",
                x[2], x[3]
            );

            // Calculate and print the objective value (profit)
            let profit: f64 = x
                .iter()
                .enumerate()
                .map(|(i, &val)| vec_c[(i, 0)] * val)
                .sum();
            println!("Profit: {:.6}", profit);

            // Check if the solution satisfies the constant product formula for both pools
            let new_reserve1_0 = reserves1[0] + x[0];
            let new_reserve1_1 = reserves1[1] - x[1] / fee;
            let cp_satisfied1 =
                (new_reserve1_0 * new_reserve1_1 >= reserves1[0] * reserves1[1] * 0.999);
            println!(
                "Pool 1 constant product formula satisfied: {}",
                cp_satisfied1
            );

            let new_reserve2_0 = reserves2[0] - x[3] / fee;
            let new_reserve2_1 = reserves2[1] + x[2];
            let cp_satisfied2 =
                (new_reserve2_0 * new_reserve2_1 >= reserves2[0] * reserves2[1] * 0.999);
            println!(
                "Pool 2 constant product formula satisfied: {}",
                cp_satisfied2
            );
        }
        Err(e) => {
            println!("Error: {:?}", e);

            // Print problem details for debugging
            println!("Objective vector c:");
            println!("{:?}", vec_c);
            println!("Constraint matrix G:");
            println!("{:?}", mat_g);
            println!("Constraint vector h:");
            println!("{:?}", vec_h);
        }
    }
}
