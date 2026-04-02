terraform {
  required_providers {
    oci = {
      source = "oracle/oci"
    }
  }
}

provider "oci" {
  region           = var.region
  tenancy_ocid     = var.tenancy_ocid
  user_ocid        = var.user_ocid
  fingerprint      = var.fingerprint
  private_key_path = var.private_key_path
}

# --- Network ---

resource "oci_core_vcn" "main" {
  cidr_block     = "10.0.0.0/16"
  compartment_id = var.compartment_id
  display_name   = "deductible-vcn"
}

resource "oci_core_internet_gateway" "main" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.main.id
  display_name   = "main-ig"
}

resource "oci_core_route_table" "main" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.main.id
  display_name   = "main-rt"
  route_rules {
    destination       = "0.0.0.0/0"
    network_entity_id = oci_core_internet_gateway.main.id
  }
}

resource "oci_core_security_list" "main" {
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.main.id
  display_name   = "main-sl"

  egress_security_rules {
    destination = "0.0.0.0/0"
    protocol    = "all"
  }

  ingress_security_rules {
    protocol = "6" # TCP
    source   = "0.0.0.0/0"
    tcp_options {
      min = 80
      max = 80
    }
  }

  ingress_security_rules {
    protocol = "6" # TCP
    source   = "0.0.0.0/0"
    tcp_options {
      min = 443
      max = 443
    }
  }

  dynamic "ingress_security_rules" {
    for_each = var.temporary_ssh_cidr == "" ? [] : [var.temporary_ssh_cidr]
    content {
      protocol = "6" # TCP
      source   = ingress_security_rules.value
      tcp_options {
        min = 22
        max = 22
      }
    }
  }
}

resource "oci_core_subnet" "public" {
  cidr_block        = "10.0.1.0/24"
  compartment_id    = var.compartment_id
  vcn_id            = oci_core_vcn.main.id
  display_name      = "public-subnet"
  route_table_id    = oci_core_route_table.main.id
  security_list_ids = [oci_core_security_list.main.id]
}

# Private subnet for Autonomous Database private endpoint (no direct internet route)
resource "oci_core_subnet" "private_db" {
  cidr_block     = "10.0.2.0/24"
  compartment_id = var.compartment_id
  vcn_id         = oci_core_vcn.main.id
  display_name   = "private-db-subnet"
  # Do not attach the internet gateway route table to keep it private
  security_list_ids = [oci_core_security_list.main.id]
}

# --- Database (ATP Free Tier) ---

resource "oci_database_autonomous_database" "free_atp" {
  compartment_id           = var.compartment_id
  subnet_id                = oci_core_subnet.private_db.id
  cpu_core_count           = 0
  data_storage_size_in_tbs = 1 # Terraform requires a whole number (TB). Free tier provides 20GB regardless.
  db_name                  = "deductibledb"
  display_name             = "deductible_db_free"
  admin_password           = var.db_admin_password
  is_free_tier             = true
  db_workload              = "OLTP"
  license_model            = "LICENSE_INCLUDED"

  # Note: Access control / whitelisted IPs are not supported for this Autonomous DB shape/type.
  # If you need to restrict public access, configure the DB's network access separately outside Terraform.

  lifecycle {
    ignore_changes = [
      is_free_tier,
      data_storage_size_in_tbs,
      cpu_core_count,
    ]
  }
}


# --- Object Storage ---

data "oci_objectstorage_namespace" "user_namespace" {
  compartment_id = var.compartment_id
}

resource "oci_objectstorage_bucket" "receipts_bucket" {
  compartment_id = var.compartment_id
  name           = "deductible_receipts"
  namespace      = data.oci_objectstorage_namespace.user_namespace.namespace
  access_type    = "NoPublicAccess"
  storage_tier   = "Standard"
}

# Create a dedicated user for the application to access storage
resource "oci_identity_user" "app_user" {
  compartment_id = var.compartment_id
  name           = "deductible_app_user"
  description    = "Service account for Deductible Tracker App"
  email          = var.app_user_email
}

resource "oci_identity_group" "app_group" {
  compartment_id = var.compartment_id
  name           = "deductible_app_group"
  description    = "Group for Deductible Tracker App"
}

resource "oci_identity_user_group_membership" "app_user_membership" {
  group_id = oci_identity_group.app_group.id
  user_id  = oci_identity_user.app_user.id
}

resource "oci_identity_policy" "app_policy" {
  compartment_id = var.compartment_id
  name           = "deductible_app_policy"
  description    = "Allow app to manage receipts bucket"
  statements = [
    "Allow group ${oci_identity_group.app_group.name} to manage objects in compartment id ${var.compartment_id} where target.bucket.name='${oci_objectstorage_bucket.receipts_bucket.name}'"
  ]
}

resource "oci_identity_customer_secret_key" "app_s3_key" {
  display_name = "deductible_app_s3_key"
  user_id      = oci_identity_user.app_user.id
}


# --- Compute Instance (ARM Ampere A1 Free Tier) ---

data "oci_identity_availability_domain" "ad1" {
  compartment_id = var.compartment_id
  ad_number      = 1
}

# Find the latest Oracle Linux 9 Image for the selected shape
data "oci_core_images" "oracle_linux_images" {
  compartment_id           = var.compartment_id
  operating_system         = "Oracle Linux"
  operating_system_version = "9"
  shape                    = var.instance_shape
  sort_by                  = "TIMECREATED"
  sort_order               = "DESC"
}

resource "oci_core_instance" "app_server" {
  availability_domain = data.oci_identity_availability_domain.ad1.name
  compartment_id      = var.compartment_id
  shape               = var.instance_shape

  lifecycle {
    ignore_changes = [
      metadata,
    ]
  }

  dynamic "shape_config" {
    for_each = length(regexall("Flex", var.instance_shape)) > 0 ? [1] : []
    content {
      ocpus         = var.instance_ocpus
      memory_in_gbs = var.instance_memory_in_gbs
    }
  }

  source_details {
    source_type             = "image"
    source_id               = data.oci_core_images.oracle_linux_images.images[0].id
    boot_volume_size_in_gbs = 50 # Within 200GB free tier limit
  }

  create_vnic_details {
    subnet_id        = oci_core_subnet.public.id
    assign_public_ip = false
  }

  metadata = {
    ssh_authorized_keys = file("${path.module}/../github_action_key.pub")
    user_data           = base64encode(file("${path.module}/cloud-init.yaml"))
  }

  defined_tags = {
    "Operations.deployed_image" = "initial"
  }
}

data "oci_core_vnic_attachments" "app_server" {
  compartment_id = var.compartment_id
  instance_id    = oci_core_instance.app_server.id
}

data "oci_core_vnic" "app_server_primary" {
  vnic_id = data.oci_core_vnic_attachments.app_server.vnic_attachments[0].vnic_id
}

data "oci_core_private_ips" "app_server_primary" {
  vnic_id = data.oci_core_vnic.app_server_primary.id
}

locals {
  app_server_primary_private_ip_ids = [
    for private_ip in data.oci_core_private_ips.app_server_primary.private_ips : private_ip.id
    if private_ip.is_primary
  ]
}

resource "oci_core_public_ip" "app_server_reserved" {
  compartment_id = var.compartment_id
  display_name   = "deductible-app-server-ip"
  lifetime       = "RESERVED"
  private_ip_id  = local.app_server_primary_private_ip_ids[0]
}

# --- IAM for Metadata Pull (Instance Principals) ---

resource "oci_identity_dynamic_group" "app_instances" {
  compartment_id = var.tenancy_ocid # Dynamic groups must be in the tenancy root
  name           = "deductible_app_instances"
  description    = "Group for Deductible Tracker app instances"
  matching_rule  = "Any {instance.id = '${oci_core_instance.app_server.id}'}"
}

resource "oci_identity_policy" "instance_metadata_policy" {
  compartment_id = var.compartment_id
  name           = "deductible_instance_metadata_policy"
  description    = "Allow instance to read its own metadata/tags"
  statements = [
    "Allow dynamic-group ${oci_identity_dynamic_group.app_instances.name} to read instances in compartment id ${var.compartment_id}"
  ]
}

# Tag Namespace for versioning/deployment
resource "oci_identity_tag_namespace" "operations" {
  compartment_id = var.compartment_id
  description    = "Operations and deployment tags"
  name           = "Operations"
}

resource "oci_identity_tag" "deployed_image" {
  description      = "Reference to the currently deployed docker image"
  name             = "deployed_image"
  tag_namespace_id = oci_identity_tag_namespace.operations.id
}

resource "oci_identity_tag" "app_secrets" {
  description      = "Base64 encoded environment variables for the app"
  name             = "app_secrets"
  tag_namespace_id = oci_identity_tag_namespace.operations.id
}
