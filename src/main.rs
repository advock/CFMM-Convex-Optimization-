use totsu::*;

fn main() {
    let global_indices = vec![0, 1, 2, 3];

    let local_indices = vec![
        vec![0, 1, 2, 3], // balancer pool with 4 tokens
        vec![0, 1],       // UniswapV2 TOKEN-0/TOKEN-1
        vec![1, 2],       // UniswapV2 TOKEN-1/TOKEN-2
        vec![2, 3],       // UniswapV2 TOKEN-2/TOKEN-3
        vec![2, 3],       // Constant Sum TOKEN-2/TOKEN-3
    ];
}
