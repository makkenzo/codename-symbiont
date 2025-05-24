use anyhow::Result;
use candle_core::{D, DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE as BERT_DTYPE};
use hf_hub::{Repo, RepoType, api::sync::Api};
use std::path::PathBuf;
use tokenizers::{EncodeInput, Tokenizer};

pub struct EmbeddingGenerator {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    config: BertConfig,
}

impl EmbeddingGenerator {
    pub fn new(model_id: &str, revision: Option<String>, force_cpu: bool) -> Result<Self> {
        let device = if force_cpu {
            Device::Cpu
        } else {
            Device::cuda_if_available(0).unwrap_or(Device::Cpu)
        };
        println!("[EmbeddingGenerator] Using device: {:?}", device);

        let api = Api::new()?;
        let repo_id = model_id.to_string();
        let revision = revision.unwrap_or_else(|| "main".to_string());
        let repo = api.repo(Repo::with_revision(repo_id, RepoType::Model, revision));

        println!("[EmbeddingGenerator] Fetching model files from Hugging Face Hub...");
        let tokenizer_filename = repo.get("tokenizer.json")?;
        let config_filename = repo.get("config.json")?;

        let model_filenames: Vec<PathBuf> = {
            if let Ok(sf_file) = repo.get("model.safetensors") {
                vec![sf_file]
            } else if let Ok(sf_index_file) = repo.get("model.safetensors.index.json") {
                let index_content: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&sf_index_file)?)?;
                let weight_map = index_content
                    .get("weight_map")
                    .ok_or_else(|| anyhow::anyhow!("No weight_map in safetensors index"))?
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("weight_map is not an object"))?;
                let mut files_to_download = std::collections::HashSet::new();
                for file_path in weight_map.values() {
                    if let Some(s) = file_path.as_str() {
                        files_to_download.insert(s.to_string());
                    }
                }
                files_to_download
                    .into_iter()
                    .map(|f| repo.get(&f))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![repo.get("pytorch_model.bin")?]
            }
        };

        println!(
            "[EmbeddingGenerator] Tokenizer path: {:?}",
            tokenizer_filename
        );
        println!("[EmbeddingGenerator] Config path: {:?}", config_filename);
        println!(
            "[EmbeddingGenerator] Model weights paths: {:?}",
            model_filenames
        );

        let config_str = std::fs::read_to_string(config_filename)?;
        let config: BertConfig = serde_json::from_str(&config_str)?;

        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(anyhow::Error::msg)?;

        let vb = unsafe {
            if model_filenames
                .iter()
                .any(|f| f.extension().map_or(false, |ext| ext == "safetensors"))
            {
                VarBuilder::from_mmaped_safetensors(&model_filenames, BERT_DTYPE, &device)?
            } else if model_filenames
                .iter()
                .any(|f| f.extension().map_or(false, |ext| ext == "bin"))
            {
                anyhow::bail!(
                    "Loading .bin weights directly might need a different VarBuilder method or conversion."
                );
            } else {
                anyhow::bail!("No .safetensors or .bin model weights found.");
            }
        };

        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
            config: config.clone(),
        })
    }

    pub fn generate_sentence_embeddings(&self, sentences: &[String]) -> Result<Vec<Vec<f32>>> {
        if sentences.is_empty() {
            return Ok(Vec::new());
        }
        println!(
            "[EmbeddingGenerator] Generating embeddings for {} sentences...",
            sentences.len()
        );

        let max_seq_len = self.config.max_position_embeddings as usize;

        let inputs: Vec<EncodeInput> = sentences.iter().map(|s| s.as_str().into()).collect();
        let encodings = self
            .tokenizer
            .encode_batch(inputs, true)
            .map_err(anyhow::Error::msg)?;

        let batch_size = encodings.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        let actual_seq_len_from_tokenizer: usize;
        if !encodings.is_empty() {
            actual_seq_len_from_tokenizer = encodings[0].get_ids().len();
            if actual_seq_len_from_tokenizer > max_seq_len {
                anyhow::bail!(
                    "Tokenizer returned sequence length {} which is greater than model max_seq_len {}",
                    actual_seq_len_from_tokenizer,
                    max_seq_len
                );
            }
        } else {
            actual_seq_len_from_tokenizer = max_seq_len;
        }

        let mut all_input_ids: Vec<u32> =
            Vec::with_capacity(batch_size * actual_seq_len_from_tokenizer);
        let mut all_attention_masks: Vec<u32> =
            Vec::with_capacity(batch_size * actual_seq_len_from_tokenizer);
        let mut all_token_type_ids: Vec<u32> =
            Vec::with_capacity(batch_size * actual_seq_len_from_tokenizer);

        for encoding in &encodings {
            all_input_ids.extend_from_slice(encoding.get_ids());
            all_attention_masks.extend_from_slice(encoding.get_attention_mask());
            all_token_type_ids.extend_from_slice(encoding.get_type_ids());
        }

        let input_ids = Tensor::from_vec(
            all_input_ids,
            (batch_size, actual_seq_len_from_tokenizer),
            &self.device,
        )?;
        let attention_mask_tensor = Tensor::from_vec(
            all_attention_masks,
            (batch_size, actual_seq_len_from_tokenizer),
            &self.device,
        )?;
        let token_type_ids = Tensor::from_vec(
            all_token_type_ids,
            (batch_size, actual_seq_len_from_tokenizer),
            &self.device,
        )?;

        println!(
            "[EmbeddingGenerator] Input tensors created (shape: [{}, {}]). Running model forward pass...",
            batch_size, actual_seq_len_from_tokenizer
        );

        let hidden_states =
            self.model
                .forward(&input_ids, &token_type_ids, Some(&attention_mask_tensor))?;

        println!("[EmbeddingGenerator] Model forward pass complete. Performing mean pooling...");

        let attention_mask_f32 = attention_mask_tensor.to_dtype(DType::F32)?;
        let attention_mask_expanded = attention_mask_f32.unsqueeze(D::Minus1)?;

        let masked_embeddings = hidden_states.broadcast_mul(&attention_mask_expanded)?;
        let sum_embeddings = masked_embeddings.sum_keepdim(1)?;

        let sum_mask = attention_mask_expanded
            .sum_keepdim(1)?
            .broadcast_add(&Tensor::from_slice(&[1e-9f32], (1, 1, 1), &self.device)?)?;

        let mean_pooled_embeddings = sum_embeddings.broadcast_div(&sum_mask)?;
        let sentence_embeddings = mean_pooled_embeddings.squeeze(1)?;

        println!(
            "[EmbeddingGenerator] Mean pooling complete. Embedding shape: {:?}",
            sentence_embeddings.dims()
        );

        sentence_embeddings
            .to_vec2::<f32>()
            .map_err(anyhow::Error::from)
    }
}
