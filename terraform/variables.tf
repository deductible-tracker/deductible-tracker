variable "compartment_id" {
  description = "OCID of the compartment"
  type        = string
}

variable "tenancy_ocid" {
  description = "OCID of the tenancy"
  type        = string
}

variable "user_ocid" {
  description = "OCID of the user calling the API"
  type        = string
}

variable "fingerprint" {
  description = "Fingerprint for the key pair being used"
  type        = string
}

variable "private_key_path" {
  description = "Path to the private key file"
  type        = string
}

variable "region" {
  description = "OCI Region"
  type        = string
  default     = "us-chicago-1"
}

variable "ssh_public_key_path" {
  description = "Path to SSH Public Key"
  type        = string
  # Default points to the repo's key file (relative to the terraform/ directory).
  # This ensures the instance's `ssh_authorized_keys` is set to your `github_action_key.pub`.
  default     = "../github_action_key.pub"
}

variable "instance_shape" {
  default = "VM.Standard.A1.Flex"
}

variable "instance_ocpus" {
  default = 1
}

variable "instance_memory_in_gbs" {
  default = 4
}

variable "db_admin_password" {
  description = "Administrator password for the ATP database"
  type        = string
  sensitive   = true
}

variable "ssh_allowed_cidr" {
  description = "CIDR block allowed to connect via SSH. Set to your IP/32 for security. Defaults to 0.0.0.0/0 (not recommended)."
  type        = string
  default     = "0.0.0.0/0"
}

variable "ssh_port" {
  description = "Custom SSH port"
  type        = number
  default     = 2222
}
