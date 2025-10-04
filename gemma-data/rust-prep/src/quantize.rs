use anyhow::Result;
use ndarray::{Array1, Array2, ArrayView2, Axis};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use byteorder::{LittleEndian, WriteBytesExt};
use rand::Rng;

/// Fast randomized PCA using random projections + power iteration
pub fn pca_transform(data: ArrayView2<f32>, target_dim: usize) -> Result<(Array2<f32>, Array1<f32>)> {
    println!("Computing randomized PCA: {} -> {} dimensions...", data.ncols(), target_dim);
    
    let (n_samples, n_features) = data.dim();
    
    // Center the data
    let mean = data.mean_axis(Axis(0)).unwrap();
    let mean_clone = mean.clone();
    let centered = &data - &mean.insert_axis(Axis(0));
    
    // Random projection matrix
    let mut rng = rand::thread_rng();
    let mut random_matrix = Array2::<f32>::zeros((n_features, target_dim));
    for i in 0..n_features {
        for j in 0..target_dim {
            random_matrix[[i, j]] = rng.gen::<f32>() - 0.5;
        }
    }
    
    // Project
    let mut projected = centered.dot(&random_matrix);
    
    // Normalize columns
    for j in 0..target_dim {
        let col = projected.column(j);
        let norm = col.dot(&col).sqrt();
        if norm > 1e-8 {
            for i in 0..n_samples {
                projected[[i, j]] /= norm;
            }
        }
    }
    
    println!("  Randomized projection complete");
    
    Ok((projected, mean_clone))
}

/// Symmetric 8-bit quantization: map [-max_abs, max_abs] to [0, 255]
pub fn quantize_8bit(data: ArrayView2<f32>) -> (Array2<u8>, Vec<f32>) {
    println!("Quantizing to 8-bit...");
    let (rows, cols) = data.dim();
    let mut quantized = Array2::<u8>::zeros((rows, cols));
    let mut scales = Vec::with_capacity(rows);
    
    for (i, row) in data.axis_iter(Axis(0)).enumerate() {
        let max_abs = row.iter()
            .map(|&x| x.abs())
            .fold(0.0f32, f32::max);
        
        let scale = if max_abs > 1e-8 {
            max_abs / 127.0
        } else {
            1.0
        };
        
        scales.push(scale);
        
        for (j, &val) in row.iter().enumerate() {
            let quantized_val = ((val / scale).clamp(-127.0, 127.0) + 128.0) as u8;
            quantized[(i, j)] = quantized_val;
        }
    }
    
    println!("  Scale range: [{:.6}, {:.6}]", 
             scales.iter().copied().fold(f32::INFINITY, f32::min),
             scales.iter().copied().fold(f32::NEG_INFINITY, f32::max));
    
    (quantized, scales)
}

/// Write binary format: header + scales + quantized data
pub fn write_quantized_embeddings<P: AsRef<Path>>(
    path: P,
    quantized: &Array2<u8>,
    scales: &[f32],
    mean: &Array1<f32>,
) -> Result<()> {
    let (vocab_size, embed_dim) = quantized.dim();
    let file = File::create(path.as_ref())?;
    let mut writer = BufWriter::new(file);
    
    // Header (32 bytes)
    writer.write_all(b"EMB8")?;           // magic (4 bytes)
    writer.write_u32::<LittleEndian>(1)?; // version
    writer.write_u32::<LittleEndian>(vocab_size as u32)?;
    writer.write_u32::<LittleEndian>(embed_dim as u32)?;
    writer.write_u32::<LittleEndian>(mean.len() as u32)?; // original dim
    writer.write_all(&[0u8; 12])?;        // reserved
    
    // PCA mean vector (original_dim * 4 bytes)
    for &val in mean.iter() {
        writer.write_f32::<LittleEndian>(val)?;
    }
    
    // Scale factors (vocab_size * 4 bytes)
    for &scale in scales.iter() {
        writer.write_f32::<LittleEndian>(scale)?;
    }
    
    // Quantized embeddings (vocab_size * embed_dim bytes)
    for row in quantized.axis_iter(Axis(0)) {
        for &val in row.iter() {
            writer.write_u8(val)?;
        }
    }
    
    writer.flush()?;
    
    println!("✓ Wrote quantized embeddings to {}", path.as_ref().display());
    println!("  Format: EMB8 v1");
    println!("  Size: {} bytes", 32 + mean.len() * 4 + vocab_size * (4 + embed_dim));
    
    Ok(())
}
