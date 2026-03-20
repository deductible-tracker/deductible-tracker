# --- Logging ---

resource "oci_logging_log_group" "app_logs" {
  compartment_id = var.compartment_id
  display_name   = "deductible_log_group"
  description    = "Log group for Deductible Tracker application"
}

resource "oci_logging_log" "instance_logs" {
  display_name = "instance_system_logs"
  log_group_id = oci_logging_log_group.app_logs.id
  log_type     = "CUSTOM"
  retention_duration = 30
}

resource "oci_logging_log" "app_custom_logs" {
  display_name = "deductible_app_logs"
  log_group_id = oci_logging_log_group.app_logs.id
  log_type     = "CUSTOM"
  retention_duration = 30
}

# Allow Instances to push logs
resource "oci_identity_policy" "instance_logging_policy" {
  compartment_id = var.compartment_id
  name           = "deductible_instance_logging_policy"
  description    = "Allow app instances to push logs to the custom log"
  statements     = [
    "Allow dynamic-group deductible_app_instances to use log-content in compartment id ${var.compartment_id}"
  ]
}

# Unified Monitoring Agent Configuration
resource "oci_logging_unified_agent_configuration" "docker_logs" {
  compartment_id = var.compartment_id
  description    = "Collect Docker container logs from the app server"
  display_name   = "docker_log_collection"
  is_enabled     = true
  
  service_configuration {
    configuration_type = "LOGGING"
    
    # Collect Docker Logs
    sources {
      source_type       = "LOG_TAIL"
      name              = "docker_container_logs"
      paths             = ["/var/lib/docker/containers/*/*.log"]
      parser {
        parser_type = "JSON"
        time_format = "%Y-%m-%dT%H:%M:%S.%LZ"
      }
    }
    
    destination {
      log_object_id = oci_logging_log.app_custom_logs.id
    }
  }
  
  group_association {
    group_list = [oci_identity_dynamic_group.app_instances.id]
  }
}

# Note: Refresh
