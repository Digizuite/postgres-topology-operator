helm upgrade postgres-topology-operator . -n postgres-operator --set operator.enable=true --set operator.imagePullPolicy=IfNotPresent --install --create-namespace

