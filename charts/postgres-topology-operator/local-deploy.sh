helm upgrade postgres-topology-operator . -n postgres-operator --set operator.enable=true --set operator.imagePullPolicy=IfNotPresent --set operator.image=ghcr.io/digizuite/postgres-topology-operator:local --install --create-namespace

