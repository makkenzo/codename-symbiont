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

        let mut tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(anyhow::Error::msg)?;

        let max_seq_len_for_tokenizer = config.max_position_embeddings as usize;
        let pad_token_str = "[PAD]";
        let pad_token_id = tokenizer
            .token_to_id(pad_token_str)
            .unwrap_or_else(|| {
                println!("[EmbeddingGenerator] WARN: '{}' token not explicitly found in tokenizer vocab by token_to_id. Assuming pad_token_id = 0.", pad_token_str);
                0 
            });
        let padding_params = tokenizers::PaddingParams {
            strategy: tokenizers::PaddingStrategy::Fixed(max_seq_len_for_tokenizer),
            direction: tokenizers::PaddingDirection::Right,
            pad_id: pad_token_id,
            pad_type_id: 0,
            pad_token: "[PAD]".to_string(),
            pad_to_multiple_of: None,
        };
        tokenizer.with_padding(Some(padding_params));

        let truncation_params = tokenizers::TruncationParams {
            max_length: max_seq_len_for_tokenizer,
            strategy: tokenizers::TruncationStrategy::LongestFirst,
            stride: 0,
            direction: tokenizers::TruncationDirection::Right,
        };
        let _ = tokenizer.with_truncation(Some(truncation_params));

        println!(
            "[EmbeddingGenerator] Tokenizer configured with padding and truncation to max_seq_len: {}",
            max_seq_len_for_tokenizer
        );

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
            "[EmbeddingGenerator] Attempting to generate embeddings for {} sentences...",
            sentences.len()
        );

        let max_seq_len = self.config.max_position_embeddings as usize;
        let mut all_generated_embeddings: Vec<Vec<f32>> = Vec::with_capacity(sentences.len());

        let processing_batch_size = 8;

        for sentence_chunk in sentences.chunks(processing_batch_size) {
            let current_batch_of_sentences: Vec<String> = sentence_chunk.iter().map(|s| s.to_string()).collect();
            let current_batch_len = current_batch_of_sentences.len();
            if current_batch_len == 0 {
                continue;
            }

             println!(
                "[EmbeddingGenerator] Processing batch of {} sentences. Max seq len: {}",
                current_batch_len, max_seq_len
            );

            let inputs: Vec<EncodeInput> = current_batch_of_sentences.iter().map(|s| s.as_str().into()).collect();
            let encodings = self
                .tokenizer
                .encode_batch(inputs, true) 
                .map_err(anyhow::Error::msg)?;

            let actual_seq_len_from_tokenizer = if !encodings.is_empty() {
                encodings[0].get_ids().len()
            } else {
                anyhow::bail!("Empty encodings for a non-empty sentence batch, this should not happen.");
            };

            if actual_seq_len_from_tokenizer != max_seq_len {
                 anyhow::bail!(
                    "Tokenizer returned sequence length {} but model/padding is configured for {}",
                    actual_seq_len_from_tokenizer, max_seq_len
                );
            }

            let mut all_input_ids: Vec<u32> = Vec::with_capacity(current_batch_len * max_seq_len);
            let mut all_attention_masks: Vec<u32> = Vec::with_capacity(current_batch_len * max_seq_len);
            let mut all_token_type_ids: Vec<u32> = Vec::with_capacity(current_batch_len * max_seq_len);

            for encoding in &encodings {
                all_input_ids.extend_from_slice(encoding.get_ids());
                all_attention_masks.extend_from_slice(encoding.get_attention_mask());
                all_token_type_ids.extend_from_slice(encoding.get_type_ids());
            }

            let input_ids = Tensor::from_vec(all_input_ids, (current_batch_len, max_seq_len), &self.device)?;
            let attention_mask_tensor = Tensor::from_vec(all_attention_masks, (current_batch_len, max_seq_len), &self.device)?;
            let token_type_ids = Tensor::from_vec(all_token_type_ids, (current_batch_len, max_seq_len), &self.device)?;

            println!(
                "[EmbeddingGenerator] Input tensors created for batch (shape: [{}, {}]). Running model forward pass...",
                current_batch_len, max_seq_len
            );

            let hidden_states = self.model.forward(&input_ids, &token_type_ids, Some(&attention_mask_tensor))?;
            println!("[EmbeddingGenerator] Model forward pass complete for batch. Performing mean pooling...");

            let attention_mask_f32 = attention_mask_tensor.to_dtype(DType::F32)?;
            let attention_mask_expanded = attention_mask_f32.unsqueeze(D::Minus1)?;
            let masked_embeddings = hidden_states.broadcast_mul(&attention_mask_expanded)?;
            let sum_embeddings = masked_embeddings.sum_keepdim(1)?;
            let sum_mask = attention_mask_expanded.sum_keepdim(1)?.broadcast_add(&Tensor::from_slice(&[1e-9f32], (1, 1, 1), &self.device)?)?;
            let mean_pooled_embeddings = sum_embeddings.broadcast_div(&sum_mask)?;
            let sentence_embeddings_tensor = mean_pooled_embeddings.squeeze(1)?;

            println!(
                "[EmbeddingGenerator] Mean pooling complete for batch. Embedding shape: {:?}",
                sentence_embeddings_tensor.dims()
            );

            let batch_embeddings_vec = sentence_embeddings_tensor.to_vec2::<f32>()?;
            all_generated_embeddings.extend(batch_embeddings_vec);
        }

        println!(
            "[EmbeddingGenerator] All batches processed. Total embeddings generated: {}",
            all_generated_embeddings.len()
        );
        Ok(all_generated_embeddings)
    }
}
