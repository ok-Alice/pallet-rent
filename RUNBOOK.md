# How to run

## Steps

- Pull docker image from docker hub

```bash
docker pull xcodecraft/pallet-rent
```

- Tag the image for gcr.io

```bash
docker tag xcodecraft/pallet-rent gcr.io/hack-at-the-edge/pallet-rent
```

- Push the image to gcr.io

```bash
docker push gcr.io/hack-at-the-edge/pallet-rent
```

- Deploy the image to Google Cloud Run

```bash
gcloud run deploy pallet-rent \
--image=gcr.io/hack-at-the-edge/pallet-rent-character-loadout@sha256:DOCKER_IMAGE_HASH \
--allow-unauthenticated \
--port=9944 \
--args=--dev,--unsafe-ws-external \
--timeout=500 \
--cpu=2 \
--memory=8Gi \
--min-instances=1 \
--max-instances=1 \
--region=europe-west1 \
--project=hack-at-the-edge
```
