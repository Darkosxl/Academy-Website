packer {
  required_version = "= 1.16.0"
  required_plugins {
    amazon = {
      source  = "github.com/hashicorp/amazon"
      version = "= 1.8.2"
    }
  }
}

variable "aws_region" {
  type = string
}

variable "source_ami" {
  type = string
  validation {
    condition     = can(regex("^ami-[0-9a-f]+$", var.source_ami))
    error_message = "Source_ami must be an AMI ID."
  }
}

variable "subnet_id" {
  type = string
}

variable "security_group_id" {
  type = string
}

variable "instance_profile_name" {
  type = string
}

variable "instance_type" {
  type    = string
  default = "r8i.4xlarge"
}

variable "source_archive" {
  type = string
}

variable "source_commit" {
  type = string
  validation {
    condition     = can(regex("^[0-9a-f]{40}$", var.source_commit))
    error_message = "Source_commit must be a full Git commit."
  }
}

locals {
  short_commit = substr(var.source_commit, 0, 12)
}

source "amazon-ebs" "worker" {
  ami_name        = "exposure-benchmark-worker-${local.short_commit}-{{timestamp}}"
  ami_description = "Verified Exposure benchmark worker at ${var.source_commit}"

  region                      = var.aws_region
  source_ami                  = var.source_ami
  instance_type               = var.instance_type
  subnet_id                   = var.subnet_id
  security_group_id           = var.security_group_id
  iam_instance_profile        = var.instance_profile_name
  associate_public_ip_address = false

  communicator              = "ssh"
  ssh_username              = "ubuntu"
  ssh_interface             = "session_manager"
  ssh_file_transfer_method  = "sftp"
  ssh_clear_authorized_keys = true
  ssh_timeout               = "20m"
  pause_before_ssm          = "20s"

  user_data_file = "${path.root}/cloud-init.yaml"

  metadata_options {
    http_endpoint               = "enabled"
    http_tokens                 = "required"
    http_put_response_hop_limit = 1
  }

  launch_block_device_mappings {
    device_name           = "/dev/sda1"
    volume_type           = "gp3"
    volume_size           = 200
    encrypted             = true
    delete_on_termination = true
  }

  tags = {
    Name         = "exposure-benchmark-image-builder"
    Service      = "exposure-benchmark"
    SourceCommit = var.source_commit
    ManagedBy    = "packer"
  }

  snapshot_tags = {
    Service      = "exposure-benchmark"
    SourceCommit = var.source_commit
    ManagedBy    = "packer"
  }
}

build {
  name    = "exposure-benchmark-worker"
  sources = ["source.amazon-ebs.worker"]

  provisioner "file" {
    source      = var.source_archive
    destination = "/tmp/exposure-source.tar.gz"
  }

  provisioner "shell" {
    timeout = "120m"
    inline = [
      "cd /",
      "sudo cloud-init status --wait",
      "sudo install -d -m 0755 -o root -g root /var/tmp/exposure-source",
      "sudo tar -xzf /tmp/exposure-source.tar.gz -C /var/tmp/exposure-source",
      "printf '%s\\n' '${var.source_commit}' | sudo tee /var/tmp/exposure-source/.deploy-commit >/dev/null",
      "sudo /var/tmp/exposure-source/infra/ec2/prepare-worker-image.sh /var/tmp/exposure-source '${var.source_commit}'",
      "sudo /var/tmp/exposure-source/infra/ec2/seal-worker-image.sh /var/tmp/exposure-source",
    ]
  }

  post-processor "manifest" {
    output     = "${path.root}/packer-manifest.json"
    strip_path = true
  }
}
