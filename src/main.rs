use totsu::prelude::*;
use totsu::{MatBuild, ProbLP, ProbSOCP};
type La = FloatGeneric<f64>;

fn main() {
    // Simple problem with one pool and two tokens
    let reserves: Vec<f64> = vec![100.0, 10.0]; // TOKEN-0/TOKEN-1
    let fee: f64 = 0.997;
    let market_value: Vec<f64> = vec![1.0, 12.0]; // Significant price discrepancy

    // Create variables: [lambda0, delta0, lambda1, delta1]
    let n_vars = 4;

    // Objective vector `c`
    let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
    vec_c.set_by_fn(|r, _| {
        if r % 2 == 0 {
            -market_value[r / 2] // For lambda (negative because we're minimizing)
        } else {
            market_value[r / 2] // For delta
        }
    });

    // Constraint matrix G and vector h for Gx <= h
    let mut mat_g = MatBuild::new(MatType::General(6, n_vars));
    let mut vec_h = MatBuild::new(MatType::General(6, 1));

    // Constraint 1: lambda0 <= x0 + delta0
    mat_g[(0, 0)] = 1.0; // lambda0
    mat_g[(0, 1)] = -1.0; // -delta0
    vec_h[(0, 0)] = reserves[0];

    // Constraint 2: lambda1 <= x1 + delta1
    mat_g[(1, 2)] = 1.0; // lambda1
    mat_g[(1, 3)] = -1.0; // -delta1
    vec_h[(1, 0)] = reserves[1];

    // Constraint 3: delta0 <= x0 (can't tender more than available)
    mat_g[(2, 1)] = 1.0; // delta0
    vec_h[(2, 0)] = reserves[0];

    // Constraint 4: delta1 <= x1 (can't tender more than available)
    mat_g[(3, 3)] = 1.0; // delta1
    vec_h[(3, 0)] = reserves[1];

    // Constraint 5 and 6: Non-negativity for all variables
    for i in 0..n_vars {
        mat_g[(4 + i / 2, i)] = -1.0;
        vec_h[(4 + i / 2, 0)] = 0.0;
    }

    // Create empty matrices for equality constraints (we don't have any in this problem)
    let mat_a = MatBuild::new(MatType::General(0, n_vars));
    let vec_b = MatBuild::new(MatType::General(0, 1));

    // Create the ProbLP struct
    let mut prob = ProbLP::new(vec_c.clone(), mat_g, vec_h, mat_a, vec_b);

    // Set up the Totsu solver
    let mut solver = Solver::<La>::new();

    // Adjust solver parameters
    solver = solver.par(|p| {
        p.eps_acc = 1e-8;
        p.eps_inf = 1e-8;
        p.max_iter = Some(1000000);
    });

    // Generate the problem components and solve
    let (op_c, op_a, op_b, cone, mut work) = prob.problem();
    match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
        Ok((x, _y)) => {
            println!("Optimal solution found:");
            println!("Token 0: lambda = {:.6}, delta = {:.6}", x[0], x[1]);
            println!("Token 1: lambda = {:.6}, delta = {:.6}", x[2], x[3]);

            // Calculate and print the objective value
            let obj_val: f64 = x
                .iter()
                .enumerate()
                .map(|(i, &val)| -vec_c[(i, 0)] * val)
                .sum();
            println!("Objective value: {:.6}", obj_val);

            // Check if the solution satisfies the constant product formula
            let new_reserve0 = reserves[0] + fee * x[1] - x[0];
            let new_reserve1 = reserves[1] + fee * x[3] - x[2];
            let constant_product_satisfied =
                (new_reserve0 * new_reserve1 >= reserves[0] * reserves[1]);
            println!(
                "Constant product formula satisfied: {}",
                constant_product_satisfied
            );
        }
        Err(e) => println!("Error: {:?}", e),
    }
}
