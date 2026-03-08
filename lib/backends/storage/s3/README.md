# harmonia-s3

## Purpose

S3-compatible bulk storage with local-mode fallback. Uploads files to S3 (or a local directory in test mode) for persistent artifact storage, backups, and media.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_s3_version` | `() -> *const c_char` | Version string |
| `harmonia_s3_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_s3_upload_file` | `(local_path: *const c_char, bucket: *const c_char, key: *const c_char) -> i32` | Upload file to S3/local |
| `harmonia_s3_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_s3_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_S3_MODE` | `local` | Storage mode: `local` or `s3` |
| `HARMONIA_S3_LOCAL_ROOT` | `$STATE_ROOT/s3-local` | Local-mode storage root |

In `s3` mode, uses `aws s3 cp` CLI (requires AWS credentials configured).

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_s3_upload_file"
  :string "/tmp/harmonia/audio.mp3"
  :string "harmonia-media"
  :string "audio/2024/recording.mp3" :int)
```

## Self-Improvement Notes

- Local mode copies files into `$HARMONIA_S3_LOCAL_ROOT/<bucket>/<key>`, creating dirs as needed.
- S3 mode shells out to `aws s3 cp`; ensure IAM credentials are available.
- To add download: implement `harmonia_s3_download_file(bucket, key, local_path)`.
- To add listing: implement `harmonia_s3_list(bucket, prefix)`.
- To replace AWS CLI with native Rust: use `aws-sdk-s3` crate.
