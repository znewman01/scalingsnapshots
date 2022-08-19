terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "4.32.0"
    }
  }

  backend "gcs" {
    bucket = "zjn-scaling-tuf-data"
  }
}

provider "google" {
  project = "zjn-scaling-tuf"
  region  = "us-central1"
  zone    = "us-central1-c"

}

locals {
  datas = [
    "gs://zjn-scaling-tuf-data/data/fakedata.tar.gz",
    "gs://zjn-scaling-tuf-data/data/fakedata2.tar.gz"
  ]
  authenticators = ["insecure", "rsa", "merkle", "mercury_hash", "mercury_diff", "mercury_hash_diff", "vanilla_tuf", "hackage"]
  things         = setproduct(local.datas, local.authenticators)
}

data "google_storage_bucket" "default" {
  name = "zjn-scaling-tuf-data"
}

resource "google_service_account" "default" {
  account_id   = "machine-creds"
  display_name = "Credentials for the simulator machines."
}

resource "google_storage_bucket_iam_binding" "binding" {
  bucket = data.google_storage_bucket.default.name
  role   = "roles/storage.admin"
  members = [
    "serviceAccount:${google_service_account.default.email}",
  ]
}

resource "google_compute_instance" "default" {
  name         = "simulator${count.index}-${replace(local.things[count.index][1], "_", "-")}"
  machine_type = "e2-highcpu-32"
  count        = length(local.things)

  boot_disk {
    initialize_params {
      image = "https://www.googleapis.com/compute/v1/projects/ubuntu-os-pro-cloud/global/images/ubuntu-pro-2204-jammy-v20220810"
    }
  }

  network_interface {
    network = "default"

    access_config {
      // Ephemeral public IP: needed to access GCS
    }
  }

  metadata = {
    data-url      = local.things[count.index][0]
    authenticator = local.things[count.index][1]
  }

  metadata_startup_script = file("startup_script.sh")

  service_account {
    email  = google_service_account.default.email
    scopes = ["cloud-platform"]
  }
}
