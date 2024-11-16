// fn main() {
//     // Reserves for each pool
//     let reserves1: Vec<f64> = vec![100.0, 10.0]; // Pool 1: TOKEN-0 / TOKEN-1
//     let reserves2: Vec<f64> = vec![90.0, 15.0]; // Pool 2: TOKEN-1 / TOKEN-2
//     let reserves3: Vec<f64> = vec![80.0, 20.0]; // Pool 3: TOKEN-2 / TOKEN-0
//     let fee: f64 = 0.997; // 0.3% fee

//     // Number of variables (x0, y1, x2, y3, x4, y5)
//     let n_vars = 6;

//     // Objective vector `c` (maximize profit)
//     let mut vec_c = MatBuild::new(MatType::General(n_vars, 1));
//     vec_c[(0, 0)] = -1.0; // Trade TOKEN-0 into Pool 1
//     vec_c[(5, 0)] = 1.0; // Receive TOKEN-0 from Pool 3

//     // Constraint matrix G and vector h for Gx <= h
//     let mut mat_g = MatBuild::new(MatType::General(12, n_vars));
//     let mut vec_h = MatBuild::new(MatType::General(12, 1));

//     // Pool 1 constraints (TOKEN-0 to TOKEN-1)
//     mat_g[(0, 0)] = reserves1[1];
//     mat_g[(0, 1)] = -reserves1[0] / fee;
//     vec_h[(0, 0)] = reserves1[0] * reserves1[1] * 0.01;

//     // Pool 2 constraints (TOKEN-1 to TOKEN-2)
//     mat_g[(1, 2)] = reserves2[1];
//     mat_g[(1, 3)] = -reserves2[0] / fee;
//     vec_h[(1, 0)] = reserves2[0] * reserves2[1] * 0.01;

//     // Pool 3 constraints (TOKEN-2 to TOKEN-0)
//     mat_g[(2, 4)] = reserves3[1];
//     mat_g[(2, 5)] = -reserves3[0] / fee;
//     vec_h[(2, 0)] = reserves3[0] * reserves3[1] * 0.01;

//     // Cycle constraints: ensure TOKEN-1 received from Pool 1 equals TOKEN-1 traded to Pool 2
//     mat_g[(3, 1)] = 1.0;
//     mat_g[(3, 2)] = -1.0;
//     vec_h[(3, 0)] = 0.0;

//     // Cycle constraints: ensure TOKEN-2 received from Pool 2 equals TOKEN-2 traded to Pool 3
//     mat_g[(4, 3)] = 1.0;
//     mat_g[(4, 4)] = -1.0;
//     vec_h[(4, 0)] = 0.0;

//     // Cycle constraints: ensure TOKEN-0 received from Pool 3 equals TOKEN-0 traded to Pool 1
//     mat_g[(5, 5)] = 1.0;
//     mat_g[(5, 0)] = -1.0;
//     vec_h[(5, 0)] = -0.0001; // Allow slight negative profit

//     // Non-negativity constraints for all trades
//     for i in 0..n_vars {
//         mat_g[(6 + i, i)] = -1.0;
//         vec_h[(6 + i, 0)] = 0.0;
//     }

//     // Create empty matrices for equality constraints
//     let mat_a = MatBuild::new(MatType::General(0, n_vars));
//     let vec_b = MatBuild::new(MatType::General(0, 1));

//     // Define and solve the optimization problem
//     let mut prob = ProbLP::new(vec_c.clone(), mat_g.clone(), vec_h.clone(), mat_a, vec_b);
//     let mut solver = Solver::<La>::new();
//     solver = solver.par(|p| {
//         p.eps_acc = 1e-12;
//         p.eps_inf = 1e-12;
//         p.eps_zero = 1e-14;
//         p.max_iter = Some(1000000);
//         p.log_period = 10000;
//     });

//     // Solve and output results
//     let (op_c, op_a, op_b, cone, mut work) = prob.problem();
//     match solver.solve((op_c, op_a, op_b, cone, &mut work)) {
//         Ok((x, _y)) => {
//             println!("Optimal solution found:");
//             println!(
//                 "Pool 1 - Trade TOKEN-0: {:.12}, Receive TOKEN-1: {:.12}",
//                 x[0], x[1]
//             );
//             println!(
//                 "Pool 2 - Trade TOKEN-1: {:.12}, Receive TOKEN-2: {:.12}",
//                 x[2], x[3]
//             );
//             println!(
//                 "Pool 3 - Trade TOKEN-2: {:.12}, Receive TOKEN-0: {:.12}",
//                 x[4], x[5]
//             );

//             // Calculate and print the objective value (profit)
//             let profit: f64 = x[5] - x[0];
//             println!("Solution vector: {:?}", x);
//             println!("Profit: {:.16}", profit);

//             // Additional checks and exchange rates
//             println!("TOKEN-1 balance: {:.12}", x[1] - x[2]);
//             println!("TOKEN-2 balance: {:.12}", x[3] - x[4]);
//             println!("TOKEN-0 balance: {:.12}", x[5] - x[0]);

//             println!(
//                 "Pool 1 exchange rate: 1 TOKEN-0 = {:.12} TOKEN-1",
//                 reserves1[1] / reserves1[0]
//             );
//             println!(
//                 "Pool 2 exchange rate: 1 TOKEN-1 = {:.12} TOKEN-2",
//                 reserves2[1] / reserves2[0]
//             );
//             println!(
//                 "Pool 3 exchange rate: 1 TOKEN-2 = {:.12} TOKEN-0",
//                 reserves3[1] / reserves3[0]
//             );
//         }
//         Err(e) => {
//             println!("Error: {:?}", e);
//             println!("Objective vector c:");
//             println!("{:?}", vec_c);
//             println!("Constraint matrix G:");
//             println!("{:?}", mat_g);
//             println!("Constraint vector h:");
//             println!("{:?}", vec_h);
//         }
//     }
// }

fn main() {
    print!("cjsbjcn s");
}
