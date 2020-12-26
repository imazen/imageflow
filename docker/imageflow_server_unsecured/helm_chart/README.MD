# Imageflow Helm Chart

This is a heml deployment chart for kubernetes.
The chart pulls and deploys the image with the configuration options in Values.yaml

## container version

If you need to upgrade or use a different version of the container please use the appVersion: setting in Chart.yaml Everytime you upgrade or change the chart values or other files you are expected to update the chartversion as well. these updates are symantic. More information can be found here https://helm.sh/docs/topics/charts/

## File structure

```
.
├── charts
│ └── [...]                             // Additional charts for dependencies could be deployed here
├── templates                           // In normal circumstances these should not be edited, Use the values.yml file instead
│ ├── Tests                             // Optional
│ |  └── [...]                          // Testcontainers and deployscripts
│ |── _helpers.tpl                      // Helper fuctions to create and maintain variables
│ |── deployment.yaml                   // Deployment of the mail app.
│ |── hpa.yaml                          // Template for pod auto scaling
│ |── ingress.yaml                      // Template for ingress creation
│ |── notes.txt                         // Generates message after template deployment
│ |── persistent-volume-claim.yaml      // Template for PVC creation
│ |── service.yaml                      // Template for service creation
│ └── serviceaccount.yaml               // Template for Service Account creation
├── .helmignore
│── Chart.yaml                          // Appversion and container version live here
│── README.MD
│── values.yaml

```

## deployment

Cd into this directory and run

```bash
helm install $deploymentname ./ --namespace $namespacename
```

### Test deployment

Generate a readable output without deploying

```bash
helm install $deploymentname ./ --namespace $namespacename --dry-run
```

### Upgrade deployment

Upgrade existing deployment

```bash
helm upgrade --install $deploymentname ./ --namespace $namespacename --dry-run
```

## Container Startup arguments.

The Values.yaml file supports an array of startup arguments. When you are ready to mount containers you can add the settings to the "startupArgs:" in the values file.

```
startupArgs: [
    "sudo chown -R imageflow /home/imageflow/; /home/imageflow/imageflow_server start \
    --bind-address 0.0.0.0 \
    --port 3000 \
    --demo \
    --data-dir /home/imageflow/data/",
  ]
```

## Ingress

This chart comes with ingress support out of the box. For ingress to work you do need to have an ingress controller installed on your cluster. The example uses an Nginx ingress with TLS, but by changing the annotations you can change this to another installed ingress controller. Ingress settings are located in Values.yaml

## live and readyness probes.

Inside of the values.yaml you are able to switch on or off the monitoring of the container, these settings are used to avoid stuck containers from traffic being routed to them. When disabled, the container will always keep running.
