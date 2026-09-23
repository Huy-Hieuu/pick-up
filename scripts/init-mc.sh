set -e

# This runs in the one-shot minio-init service. Never allow anonymous writes.
mc alias set local "${MINIO_ENDPOINT:-http://minio:9000}" "$MINIO_ROOT_USER" "$MINIO_ROOT_PASSWORD"
echo "Waiting for MinIO to be ready..."
until mc ready local > /dev/null 2>&1; do
  sleep 1
done

echo "MinIO is ready. Starting initialization..."

# Create bucket if not exists
mc mb local/pickup-media --ignore-existing

mc anonymous set download local/pickup-media

echo "MinIO bucket initialized: public reads, authenticated uploads only"
