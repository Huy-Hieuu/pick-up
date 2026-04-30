set -e

echo "Waiting for MinIO to be ready..."
until mc ready local > /dev/null 2>&1; do
  sleep 1
done

echo "MinIO is ready. Starting initialization..."

# Create alias
mc alias set local http://localhost:9000 minioadmin minioadmin

# Create bucket if not exists
mc mb local/pickup-media --ignore-existing

cat > /tmp/policy.json << 'EOF'
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "AllowUpload",
      "Effect": "Allow",
      "Principal": {"AWS": ["*"]},
      "Action": ["s3:PutObject"],
      "Resource": "arn:aws:s3:::pickup-media/*"
    },
    {
      "Sid": "AllowDownload",
      "Effect": "Allow",
      "Principal": {"AWS": ["*"]},
      "Action": ["s3:GetObject"],
      "Resource": "arn:aws:s3:::pickup-media/*"
    }
  ]
}
EOF

mc anonymous set-json /tmp/policy.json local/pickup-media

echo "MinIO bucket 'pickup-media' initialized with public upload/download policies"