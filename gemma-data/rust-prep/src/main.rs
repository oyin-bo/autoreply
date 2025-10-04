use anyhow::{Context, Result};
use ndarray::Array2;
use safetensors::{SafeTensors, Dtype};
use std::fs;
use std::path::PathBuf;
use half::bf16;

mod quantize;

fn try_convert_tensor_to_array2_f32(bytes: &[u8], shape: &[usize], dtype: Dtype) -> Option<Array2<f32>> {
    if shape.len() != 2 {
        return None;
    }
    
    let rows = shape[0];
    let cols = shape[1];
    let total = rows * cols;
    
    let v: Vec<f32> = match dtype {
        Dtype::F32 => {
            if bytes.len() != total * 4 {
                return None;
            }
            let mut v = Vec::with_capacity(total);
            for i in (0..bytes.len()).step_by(4) {
                let val = f32::from_le_bytes([bytes[i], bytes[i+1], bytes[i+2], bytes[i+3]]);
                v.push(val);
            }
            v
        }
        Dtype::BF16 => {
            if bytes.len() != total * 2 {
                return None;
            }
            let mut v = Vec::with_capacity(total);
            for i in (0..bytes.len()).step_by(2) {
                let bits = u16::from_le_bytes([bytes[i], bytes[i+1]]);
                let bf = bf16::from_bits(bits);
                v.push(bf.to_f32());
            }
            v
        }
        Dtype::F16 => {
            if bytes.len() != total * 2 {
                return None;
            }
            let mut v = Vec::with_capacity(total);
            for i in (0..bytes.len()).step_by(2) {
                let bits = u16::from_le_bytes([bytes[i], bytes[i+1]]);
                let hf = half::f16::from_bits(bits);
                v.push(hf.to_f32());
            }
            v
        }
        _ => return None,
    };
    
    Array2::from_shape_vec((rows, cols), v).ok()
}

fn main() -> Result<()> {
    let gemma_data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let model_path = gemma_data_dir.join("model.safetensors");
    let out_npy = gemma_data_dir.join("embeddings.npy");
    let out_bin = gemma_data_dir.join("embeddings_64d_q8.bin");

    println!("Reading safetensors from: {}", model_path.display());
    
    let data = fs::read(&model_path)
        .with_context(|| format!("failed to read safetensors file {:?}", model_path))?;
    
    let st = SafeTensors::deserialize(&data)
        .with_context(|| "failed to parse safetensors")?;

    let names = st.names();
    println!("Available tensors ({}):", names.len());
    for name in &names {
        println!("  {}", name);
    }

    // Try common candidate names
    let candidates = [
        "model.embed_tokens.weight",
        "transformer.wte.weight",
        "wte.weight",
        "embed_tokens.weight",
        "model.decoder.embed_tokens.weight",
    ];

    let mut found = false;
    for cand in candidates.iter() {
        let cand_str = cand.to_string();
        if names.iter().any(|n| n == cand) {
            let view = st.tensor(cand).with_context(|| format!("failed to get tensor {}", cand))?;
            let shape = view.shape();
            let bytes = view.data();
            let dtype = view.dtype();
            println!("Found tensor '{}' shape={:?} dtype={:?}", cand, shape, dtype);
            
            if let Some(arr) = try_convert_tensor_to_array2_f32(bytes, shape, dtype) {
                // Save raw f32 version
                ndarray_npy::write_npy(&out_npy, &arr).with_context(|| format!("failed to write {}", out_npy.display()))?;
                println!("✓ Wrote raw embeddings to {}", out_npy.display());
                println!("  Shape: {:?}", arr.dim());
                
                // PCA + quantization
                println!("\nApplying PCA + 8-bit quantization...");
                let (transformed, mean) = quantize::pca_transform(arr.view(), 64)?;
                let (quantized, scales) = quantize::quantize_8bit(transformed.view());
                quantize::write_quantized_embeddings(&out_bin, &quantized, &scales, &mean)?;
                
                found = true;
                break;
            } else {
                anyhow::bail!("tensor {} not f32 2-D or unexpected size", cand);
            }
        }
    }

    if !found {
        anyhow::bail!("embedding tensor not found among common keys; inspect listed keys and adjust candidate list");
    }

    Ok(())
}
