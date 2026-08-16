#!/bin/bash
set -e

# Default to dslim/bert-base-NER
MODEL_REPO=${1:-"dslim/bert-base-NER"}
TARGET_DIR="models"

echo "Downloading model and tokenizer from $MODEL_REPO..."

mkdir -p $TARGET_DIR

# Download model.onnx (or fallback to pytorch if not available natively, but we assume it is or use Isotonic/deberta-v3-base_finetuned_ai4privacy_v2)
# Since dslim/bert-base-NER might not have model.onnx, let's use a known ONNX repo or huggingface-cli to download specific files
# For the sake of this setup, we'll download directly from HuggingFace Hub via curl

curl -L -o $TARGET_DIR/model.onnx "https://huggingface.co/$MODEL_REPO/resolve/main/model.onnx"
curl -L -o $TARGET_DIR/tokenizer.json "https://huggingface.co/$MODEL_REPO/resolve/main/tokenizer.json"
curl -L -o $TARGET_DIR/config.json "https://huggingface.co/$MODEL_REPO/resolve/main/config.json"

echo "Download complete."
