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

variable "app_user_email" {
  description = "Email address for the OCI identity user used by the Deductible Tracker App."
  type        = string
}

variable "temporary_ssh_cidr" {
  description = "Temporary SSH ingress CIDR for emergency debugging; leave empty to disable SSH ingress"
  type        = string
  default     = ""
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
