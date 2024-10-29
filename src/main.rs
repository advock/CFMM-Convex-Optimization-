use totsu::prelude::*;
use totsu::{MatBuild, ProbLP, ProbSOCP};
type La = FloatGeneric<f64>;

fn main() {
    //b
    let reserves1: Vec<f64> = vec![100.0, 10.0]; // Pool 1: TOKEN-0/TOKEN-1
    let reserves2: Vec<f64> = vec![90.0, 15.0]; // Pool 2: TOKEN-0/TOKEN-1
    let fee: f64 = 0.997; // 0.3% fee

    let n_vars = 4;

    // Objective vector `c` (maximize profit)
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
    vec_c[(0, 0)] = -1.0; // Trade TOKEN-0 to Pool 1
    vec_c[(3, 0)] = 1.0; // Receive TOKEN-0 from Pool 2

    // Constraint matrix G and vector h for Gx <= h
    let mut mat_g = MatBuild::new(MatType::General(9, n_vars)); // Increase size to 10x4 // constrains
    let mut vec_h = MatBuild::new(MatType::General(9, 1)); // Increase size to 10x1

    // Constant product constraints (slightly relaxed)
    mat_g[(0, 0)] = reserves1[1];
    mat_g[(0, 1)] = -reserves1[0] / fee;
    vec_h[(0, 0)] = reserves1[0] * reserves1[1] * 0.01; // Allow for 0.01% slippage

    mat_g[(1, 2)] = reserves2[0];
    mat_g[(1, 3)] = -reserves2[1] / fee;
    vec_h[(1, 0)] = reserves2[0] * reserves2[1] * 0.01; // Allow for 0.01% slippage

    // Ensure TOKEN-1 received from Pool 1 equals TOKEN-1 traded to Pool 2
    mat_g[(2, 1)] = 1.0;
    mat_g[(2, 2)] = -1.0;
    vec_h[(2, 0)] = 0.0; // y1 = x2

    // Ensure profit is non-negative (slightly relaxed)
    mat_g[(3, 0)] = 1.0;
    mat_g[(3, 3)] = -1.0;
    vec_h[(3, 0)] = -0.0001; // Allow for very small negative profit

    mat_g[(8, 1)] = -1.0; // Negative coefficient for output of Pool 1 (x[1])
    mat_g[(8, 2)] = 1.0; // Positive coefficient for input of Pool 2 (x[2])
    vec_h[(8, 0)] = 0.0; // Right-hand side of the inequality (x[2] - x[1] <= 0)

    // Non-negativity constraints
    for i in 0..n_vars {
        mat_g[(4 + i, i)] = -1.0;
        vec_h[(4 + i, 0)] = 0.0;
    }

    // Create empty matrices for equality constraints (we don't have any in this problem)
    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    // Create the ProbLP struct
    let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);

    // Set up the Totsu solver
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-12;
        p.eps_inf = 1e-12;
        p.eps_zero = 1e-14;
        p.max_iter = Some(1000000);
        p.log_period = 10000;
    });

    // Generate the problem components and solve
    let (op_c, op_a, op_b, cone, mut work) = prob.problem();
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
        Ok((x, _y)) => {
            println!("Optimal solution found:");
            println!(
                "Pool 1 - Trade TOKEN-0: {:.12}, Receive TOKEN-1: {:.12}",
                x[0], x[1]
            );
            println!(
                "Pool 2 - Trade TOKEN-1: {:.12}, Receive TOKEN-0: {:.12}",
                x[2], x[3]
            );

            // Calculate and print the objective value (profit)
            let profit: f64 = x[3] - x[0];
            println!("Solution vector: {:?}", x);
            println!("Profit: {:.16}", profit);

            // Additional checks
            println!("TOKEN-1 received from Pool 1: {:.12}", x[1]);
            println!("TOKEN-1 traded to Pool 2: {:.12}", x[2]);
            println!("TOKEN-1 balance: {:.12}", x[1] - x[2]);

            // Check if the solution satisfies the constant product formula for both pools
            let new_reserve1_0 = reserves1[0] + x[0];
            let new_reserve1_1 = reserves1[1] - x[1] / fee;
            let cp_satisfied1 =
                (new_reserve1_0 * new_reserve1_1 >= reserves1[0] * reserves1[1] * 0.9999);
            println!(
                "Pool 1 constant product formula satisfied: {}",
                cp_satisfied1
            );
            println!(
                "Pool 1 new reserves: {:.12}, {:.12}",
                new_reserve1_0, new_reserve1_1
            );
            println!(
                "Pool 1 constant product: before = {:.12}, after = {:.12}",
                reserves1[0] * reserves1[1],
                new_reserve1_0 * new_reserve1_1
            );

            let new_reserve2_0 = reserves2[0] - x[3] / fee;
            let new_reserve2_1 = reserves2[1] + x[2];
            let cp_satisfied2 =
                (new_reserve2_0 * new_reserve2_1 >= reserves2[0] * reserves2[1] * 0.9999);
            println!(
                "Pool 2 constant product formula satisfied: {}",
                cp_satisfied2
            );
            println!(
                "Pool 2 new reserves: {:.12}, {:.12}",
                new_reserve2_0, new_reserve2_1
            );
            println!(
                "Pool 2 constant product: before = {:.12}, after = {:.12}",
                reserves2[0] * reserves2[1],
                new_reserve2_0 * new_reserve2_1
            );

            // Calculate implied exchange rates
            println!(
                "Pool 1 exchange rate: 1 TOKEN-0 = {:.12} TOKEN-1",
                reserves1[1] / reserves1[0]
            );
            println!(
                "Pool 2 exchange rate: 1 TOKEN-0 = {:.12} TOKEN-1",
                reserves2[1] / reserves2[0]
            );

            // Manual arbitrage check
            let token1_from_pool1 = reserves1[1] * x[0] / (reserves1[0] + x[0]);
            let token0_from_pool2 =
                reserves2[0] * token1_from_pool1 / (reserves2[1] + token1_from_pool1);
            let manual_profit = token0_from_pool2 * fee - x[0];
            println!("Manual arbitrage check - Profit: {:.12}", manual_profit);
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
