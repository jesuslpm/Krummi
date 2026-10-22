# Fases de ODD

Organic Driven Development (ODD) recorre siete fases, en este orden, en cada petición.

| # | Fase | Descripción |
|---|---|---|
| 1 | Authorize | Determinar si la petición autoriza modificar código. Las consultas de solo lectura no producen cambios. |
| 2 | Explore | Examinar el código existente y los requisitos, con una profundidad proporcional al trabajo. |
| 3 | Resolve uncertainty | Investigar incertidumbres y preguntar al usuario solo cuando haya una decisión real que tomar. |
| 4 | Classify | Clasificar el trabajo como pequeño o grande según comprensión, riesgo y capacidad de reanudarlo. |
| 5 | Track before the first write | Para trabajos sustanciales, crear un documento de la funcionalidad y su copia en Engram antes de modificar código. |
| 6 | Implement task by task | Implementar cada tarea, aplicar las pruebas correspondientes, verificar resultados y crear commits revisables. |
| 7 | Close | Informar del resultado verificado, comprobaciones fallidas o pendientes y siguientes pasos. |
