provider "google" {
  project = "zjn-chainguard"
  region  = "us-east1"
  zone    = "us-east1-b"
}

provider "external" {
}

terraform {
  backend "gcs" {
    # Remote backend for tf state
    bucket = "zjn-chainguard-tf"
    prefix = "/environments/fetchaur/"
  }
}

data "external" "imgfile" {
  program = ["/bin/sh", "build_from_tf.sh"]
}

resource "google_storage_bucket" "imgbucket" {
  name          = "zjn-chainguard-aur-images"
  location      = "US"
  force_destroy = true
}

resource "google_storage_bucket_object" "img" {
  name   = "${filesha256(data.external.imgfile.result.path)}.tar.gz"
  source = data.external.imgfile.result.path
  bucket = google_storage_bucket.imgbucket.id
}


resource "google_compute_image" "cron" {
  name = "cron"
  raw_disk {
    source = google_storage_bucket_object.img.self_link
  }
}

resource "google_compute_disk" "metadata" {
  name = "metadata"
  size = 150
  lifecycle { prevent_destroy = true }
}

resource "google_compute_instance" "aurpoller" {
  name         = "aurpoller"
  machine_type = "e2-micro"

  boot_disk {
    initialize_params {
      image = google_compute_image.cron.id
    }
  }

  attached_disk {
    source      = google_compute_disk.metadata.self_link
    device_name = "metadata"
    mode        = "READ_WRITE"
  }

  metadata = {
    url = "https://sigstore.dev"
  }

  network_interface {
    network = "default"
    access_config {}
  }
}


output "public_ip" {
  value = google_compute_instance.aurpoller.network_interface.0.access_config.0.nat_ip
}
