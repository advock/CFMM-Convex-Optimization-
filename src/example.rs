// use ndarray::{Array1, Array2};
// use totsu::prelude::*;
// use totsu::*;

// struct Pool {
//     fee: f64,
// }

// impl Pool {
//     fn calculate_profit(&self, input: f64) -> f64 {
//         input * (1.0 - self.fee)
//     }
// }

// fn main() {
//     // Define some example cycles (paths), where each path is a vector of `Pool` structs
//     let detected_cycles: Vec<Vec<Pool>> = vec![
//         vec![Pool { fee: 0.003 }, Pool { fee: 0.003 }], // Path 1
//         vec![Pool { fee: 0.003 }, Pool { fee: 0.0025 }], // Path 2
//                                                         // Add more paths as needed
//     ];

//     let num_paths = detected_cycles.len();

//     // Define the profit vector based on paths; profits are initially variables
//     let mut profits = Array1::<f64>::zeros(num_paths);

//     // Set up profits based on each path
//     for (i, cycle) in detected_cycles.iter().enumerate() {
//         let mut input = 1.0; // Assume an initial input amount
//         for pool in cycle {
//             input = pool.calculate_profit(input);
//         }
//         profits[i] = input; // Store the profit for this path
//     }

//     // Define the objective matrix (negate profits to maximize when minimized)
//     let objective = -profits; // Array1<f64> representing linear coefficients in the objective

//     // Set up the constraint matrices
//     // Constraint matrix to ensure only one path is active
//     let mut a_matrix = Array2::<f64>::zeros((1, num_paths));
//     let b = Array1::<f64>::ones(1); // Right-hand side of the constraint, sum(z) == 1

//     // Fill the constraint matrix with 1's across each path
//     for i in 0..num_paths {
//         a_matrix[[0, i]] = 1.0;
//     }

//     // Initialize the conic problem solver (ConeProg) with Totsu’s format
//     let mut solver = ConeProg::<FloatGeneric<f64>>::new();

//     // Define linear programming constraints for the selection
//     let lp = LinAlg::from_constraints(objective, Some(a_matrix), Some(b), None);

//     // Solve the problem using the conic programming solver
//     let result = solver.solve(&lp);

//     match result {
//         Ok(solution) => {
//             println!("Optimal path selection:");
//             for (i, &path_profit) in profits.iter().enumerate() {
//                 let is_selected = solution[i] > 0.5;
//                 if is_selected {
//                     println!("Selected Path {}: Profit = {}", i, path_profit);
//                 }
//             }
//         }
//         Err(e) => eprintln!("Optimization failed: {:?}", e),
//     }
// }
