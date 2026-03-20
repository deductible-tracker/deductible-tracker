output "instance_public_ip" {
  value = oci_core_instance.app_server.public_ip
}

output "db_connection_string" {
  value = oci_database_autonomous_database.free_atp.connection_strings[0].profiles[0].value
}

output "object_storage_namespace" {
  value = data.oci_objectstorage_namespace.user_namespace.namespace
}

output "object_storage_bucket_name" {
  value = oci_objectstorage_bucket.receipts_bucket.name
}

output "object_storage_endpoint" {
  value = "https://${data.oci_objectstorage_namespace.user_namespace.namespace}.compat.objectstorage.${var.region}.oraclecloud.com"
}

output "oci_access_key_id" {
  value = oci_identity_customer_secret_key.app_s3_key.id
}

output "oci_secret_access_key" {
  value     = oci_identity_customer_secret_key.app_s3_key.key
  sensitive = true
}
