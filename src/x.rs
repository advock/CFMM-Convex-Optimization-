fn main() {
    let reserves1: Vec<f64> = vec![100.0, 10.0]; // Pool 1: TOKEN-0/TOKEN-1
    let reserves2: Vec<f64> = vec![90.0, 15.0]; // Pool 2: TOKEN-0/TOKEN-1
    let fee: f64 = 0.997; // 0.3% fee

    let n_vars = 4;

    // Objective vector `c` (maximize profit)
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
    vec_c[(0, 0)] = -1.0; // Trade TOKEN-0 to Pool 1
    vec_c[(3, 0)] = 1.0; // Receive TOKEN-0 from Pool 2

    // Constraint matrix G and vector h for Gx <= h
    let mut mat_g = MatBuild::new(MatType::General(10, n_vars));
    let mut vec_h = MatBuild::new(MatType::General(10, 1));

    // Constant product constraints (adjusted)
    mat_g[(0, 0)] = reserves1[1];
    mat_g[(0, 1)] = -reserves1[0];
    vec_h[(0, 0)] = reserves1[0] * reserves1[1] * (1.0 / fee - 1.0); // (x0 + dx0) * (x1 - dy1/fee) >= x0 * x1
    mat_g[(1, 2)] = reserves2[1];
    mat_g[(1, 3)] = -reserves2[0];
    vec_h[(1, 0)] = reserves2[0] * reserves2[1] * (1.0 / fee - 1.0); // (y0 - dy0/fee) * (y1 + dy1) >= y0 * y1

    // Arbitrage constraints
    mat_g[(2, 1)] = 1.0;
    mat_g[(2, 2)] = -1.0;
    vec_h[(2, 0)] = 0.0; // dy1 <= dy2
    mat_g[(3, 3)] = 1.0;
    mat_g[(3, 0)] = -1.0;
    vec_h[(3, 0)] = 0.0; // dy0 <= dx0

    // Non-negativity constraints
    for i in 0..n_vars {
        mat_g[(4 + i, i)] = -1.0;
        vec_h[(4 + i, 0)] = 0.0;
    }

    // Upper bound constraints
    mat_g[(8, 0)] = 1.0;
    vec_h[(8, 0)] = reserves1[0] * 0.1; // dx0 <= 10% of reserves1[0]
    mat_g[(9, 2)] = 1.0;
    vec_h[(9, 0)] = reserves2[1] * 0.1; // dy2 <= 10% of reserves2[1]

    // Create empty matrices for equality constraints (we don't have any in this problem)
    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    // Debug output
    println!("Objective vector c:");
    println!("{:?}", vec_c);
    println!("Constraint matrix G:");
    println!("{:?}", mat_g);
    println!("Constraint vector h:");
    println!("{:?}", vec_h);

    // Create the ProbLP struct
    let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);

    // Set up the Totsu solver
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-10;
        p.eps_inf = 1e-10;
        p.eps_zero = 1e-12;
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
            let profit: f64 = x[3] - x[0];
            println!("Profit: {:.6}", profit);

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
                "Pool 1 new reserves: {:.6}, {:.6}",
                new_reserve1_0, new_reserve1_1
            );
            println!(
                "Pool 1 constant product: before = {:.6}, after = {:.6}",
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
                "Pool 2 new reserves: {:.6}, {:.6}",
                new_reserve2_0, new_reserve2_1
            );
            println!(
                "Pool 2 constant product: before = {:.6}, after = {:.6}",
                reserves2[0] * reserves2[1],
                new_reserve2_0 * new_reserve2_1
            );

            // Calculate implied exchange rates
            println!(
                "Pool 1 exchange rate: 1 TOKEN-0 = {:.6} TOKEN-1",
                reserves1[1] / reserves1[0]
            );
            println!(
                "Pool 2 exchange rate: 1 TOKEN-0 = {:.6} TOKEN-1",
                reserves2[1] / reserves2[0]
            );

            // Manual arbitrage check
            let token1_from_pool1 = reserves1[1] * x[0] / (reserves1[0] + x[0]);
            let token0_from_pool2 =
                reserves2[0] * token1_from_pool1 / (reserves2[1] + token1_from_pool1);
            let manual_profit = token0_from_pool2 * fee - x[0];
            println!("Manual arbitrage check - Profit: {:.6}", manual_profit);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}