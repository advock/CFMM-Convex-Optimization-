use totsu::prelude::*;
use totsu::*;

type La = FloatGeneric<f64>;
type ASolver = Solver<La>;

fn main() {
    const N_TOKENS: usize = 4; //n
    const N_CFMMS: usize = 5; // m

    let global_indices = vec![0, 1, 2, 3];

    let local_indices = vec![
        vec![0, 1, 2, 3], // balancer pool with 4 tokens
        vec![0, 1],       // UniswapV2 TOKEN-0/TOKEN-1
        vec![1, 2],       // UniswapV2 TOKEN-1/TOKEN-2
        vec![2, 3],       // UniswapV2 TOKEN-2/TOKEN-3
        vec![2, 3],       // Constant Sum TOKEN-2/TOKEN-3
    ];

    let reserves = vec![
        vec![4.0, 4.0, 4.0, 4.0], // balancer
        vec![10.0, 1.0],          // UniswapV2 TOKEN-0/TOKEN-1
        vec![1.0, 5.0],           // UniswapV2 TOKEN-1/TOKEN-2
        vec![40.0, 50.0],         // UniswapV2 TOKEN-2/TOKEN-3
        vec![10.0, 10.0],         // Constant Sum TOKEN-2/TOKEN-3
    ];

    let fees = vec![0.998, 0.997, 0.997, 0.997, 0.999]; // Pool fees
    let market_value = vec![1.5, 10.0, 2.0, 3.0];

    // Variables: deltas and lambdas
    let mut deltas: Vec<Vec<f64>> = vec![vec![0.0; N_TOKENS]; N_CFMMS];
    let mut lambdas: Vec<Vec<f64>> = vec![vec![0.0; N_TOKENS]; N_CFMMS];

    let mut objective = 0.0;
    for i in 0..N_TOKENS {
        objective += market_value[i] * (lambdas[i][0] - deltas[i][0]); // Simplified case
    }

    let mut constraints = Vec::new();

    // Add pool constraints (geometric mean, sum, etc.)
    for i in 0..N_CFMMS {
        let mut pool_reserves = reserves[i].clone();
        let new_reserves: Vec<f64> = pool_reserves
            .iter()
            .enumerate()
            .map(|(j, r)| r + fees[i] * deltas[i][j] - lambdas[i][j])
            .collect();

        // Example constraint for geometric mean in Balancer pool
        if i == 0 {
            let geo_mean_original =
                (reserves[0][0] * reserves[0][1] * reserves[0][2] * reserves[0][3]).sqrt();
            let geo_mean_new =
                (new_reserves[0] * new_reserves[1] * new_reserves[2] * new_reserves[3]).sqrt();
            constraints.push(geo_mean_new >= geo_mean_original);
        }

        // Example constraint for Uniswap pools
        if i > 0 && i < 4 {
            let geo_mean_original = (reserves[i][0] * reserves[i][1]).sqrt();
            let geo_mean_new = (new_reserves[0] * new_reserves[1]).sqrt();
            constraints.push(geo_mean_new >= geo_mean_original);
        }

        // Example for constant sum
        if i == 4 {
            let sum_reserves = new_reserves.iter().sum::<f64>();
            constraints.push(sum_reserves >= reserves[4].iter().sum::<f64>());
        }
    }

    let solver = ASolver::new();
}
