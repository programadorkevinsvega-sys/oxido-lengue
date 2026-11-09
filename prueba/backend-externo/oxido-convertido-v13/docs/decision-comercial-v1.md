# Decisión comercial v1: pagos, créditos y almacenamiento

**Estado:** política inicial aprobada para diseño e implementación; los precios y tarifas son versionados. La activación de cobros reales depende de cuentas, contratos y validación fiscal.

## 1. Monedas y procesadores

- **Colombia:** catálogo y cobro en COP mediante Wompi.
- **Clientes internacionales:** usar la cuenta Stripe que el propietario confirma que ya tiene, para catálogo/cobros en USD, una vez que el backend valide estado activo, país de la cuenta merchant, monedas/liquidación, métodos habilitados, tarifas, credenciales y webhooks. No se vuelve a tratar la existencia de una cuenta como pendiente; sí se debe comprobar su configuración antes de producción.
- Mantener precios por moneda en catálogos versionados. No convertir cada compra/renovación con una tasa FX viva. Finanzas fija y publica versiones COP/USD y vigencias.
- El backend obtiene/configura tarifas contractuales vigentes por cuenta, país, método, moneda y producto. No codificar una comisión universal; conciliar liquidaciones y cargos reales.

## 2. Catálogo inicial de modelos Cloudflare

El catálogo inicial se divide en dos canales que no deben mezclarse. Los diez modelos `@cf/...` de la primera tabla son **Cloudflare-hosted** y se ejecutan por Workers AI. Los cuatro de la segunda son **Third-party** y se ejecutan mediante catálogo Cloudflare AI/AI Gateway. “Partner” identifica al proveedor/autor del modelo, pero el tag Cloudflare-hosted y el ID `@cf/...` indican que el runtime es Workers AI; no son la misma modalidad que un modelo externo BYOK.

Workers AI presenta unidades/precios por modelo, pero factura en el backend como neurons. El plan Workers Free ofrece 10.000 neurons por cuenta al día; por encima, Workers Paid cobra USD 0,011/1.000 neurons. Esa cuota es global de la cuenta Cloudflare y no se asigna como crédito diario separado a cada tenant ni reduce el costo unitario usado para vender. Puede pagarse Workers AI con créditos prepagados de AI Gateway si se configura Unified Billing; en ese caso se registra el cargo del 5 % de compra una sola vez, separado de inferencia. La tabla refleja los precios públicos consultados el 18-oct-2026 y se debe versionar/actualizar desde la cuenta y tarifario vigente.

### Workers AI — alojados por Cloudflare

| Modelo / ID Workers AI | Uso destacado | Unidad/precio público consultado |
|---|---|---|
| FLUX.2 Klein 9B — `@cf/black-forest-labs/flux-2-klein-9b` | Calidad mejorada, generación y edición con referencias | USD 0,015 primer MP de salida; USD 0,002 MP de salida adicional y por MP de imagen de entrada |
| FLUX.2 Klein 4B — `@cf/black-forest-labs/flux-2-klein-4b` | Generación/edición rápida | USD 0,000059 por tile 512×512 de entrada; USD 0,000287 por tile de salida |
| FLUX.2 Dev — `@cf/black-forest-labs/flux-2-dev` | Alta fidelidad y referencias múltiples | USD 0,00021/tile de entrada/paso + USD 0,00041/tile de salida/paso |
| Lucid Origin — `@cf/leonardo/lucid-origin` | Diseño gráfico y seguimiento preciso del prompt | USD 0,006996 por tile 512×512 + USD 0,000132 por paso |
| Phoenix 1.0 — `@cf/leonardo/phoenix-1.0` | Adherencia al prompt y texto coherente en imagen | USD 0,005830 por tile 512×512 + USD 0,000110 por paso |
| FLUX.1 Schnell — `@cf/black-forest-labs/flux-1-schnell` | Borradores rápidos, cuatro pasos por defecto | USD 0,0000528 por tile 512×512 + USD 0,0001056 por paso |
| SDXL-Lightning — `@cf/bytedance/stable-diffusion-xl-lightning` | Salida rápida 1024 px; modelo Beta | La página del modelo indica USD 0,00/paso; verificar costo efectivo y estado de disponibilidad antes de activarlo |
| DreamShaper 8 LCM — `@cf/lykon/dreamshaper-8-lcm` | Estilo fotorealista | La página consultada no muestra unidad/precio; no habilitar cobro hasta verificar tarifa/neurons reales |
| Stable Diffusion v1.5 Inpainting — `@cf/runwayml/stable-diffusion-v1-5-inpainting` | Relleno/edición con máscara; Beta | La página del modelo indica USD 0,00/paso; verificar costo efectivo y condiciones Beta antes de activarlo |
| SDXL Base 1.0 — `@cf/stabilityai/stable-diffusion-xl-base-1.0` | Generación y modificación; Beta | La página del modelo indica USD 0,00/paso; verificar costo efectivo y condiciones Beta antes de activarlo |

Las etiquetas “USD 0,00/paso” o precio no publicado no bastan para concluir que la ejecución no tenga costo o esté disponible para uso comercial. No fijar precio cero en XOVYRA sin confirmar condiciones, factura real, unidades medidas, términos y permisos de la cuenta. Los modelos Beta o sin tarifa oficial confirmada permanecen deshabilitados para clientes hasta pasar esa revisión.

### Modelos terceros — catálogo Cloudflare AI/AI Gateway

Estos cuatro modelos aparecen en el catálogo como **Third-party**. No son modelos Workers AI, no usan su cuota diaria de neurons ni su tarifa de USD 0,011/1.000 neurons. La facturación depende de Unified Billing o BYOK configurado en la cuenta.

| Modelo / ID oficial | Capacidad inicial | Tarifa publicada por Cloudflare AI al 18-oct-2026 |
|---|---|---|
| FLUX 3 Image Klein — `black-forest-labs/flux-3-image-klein` | Texto a imagen, edición, referencias múltiples | USD 0,009 por primer megapíxel de salida + USD 0,001667 por cada megapíxel de salida adicional; entrada USD 0,00/MP |
| P-Image — `pruna/p-image` | Texto a imagen | USD 0,005 por imagen |
| P-Video — `pruna/p-video` | Texto/imagen/audio a video | 720p USD 0,02/segundo; 1080p USD 0,04/segundo; borrador 720p USD 0,005/segundo; borrador 1080p USD 0,01/segundo |
| MiniMax H3 Max — `minimax/h3-max` | Texto/imagen a video | 480p USD 0,05/segundo; 768p USD 0,08/segundo |

Son tarifas publicadas consultadas en esa fecha, no precios que deban quedar fijos en código o usarse sin confirmación del catálogo/contrato. El costo FLUX cambia con los megapíxeles de salida; los costos de video cambian con segundos, resolución y modo. Antes de iniciar cada trabajo, validar parámetros reales y reservar el costo máximo aceptado.

Si la cuenta usa **Unified Billing** de Cloudflare, el costo del proveedor se traslada a la tarifa publicada sin recargo de inferencia, pero Cloudflare aplica un **5 % al comprar créditos de AI Gateway Unified Billing** (por cada USD 100 de crédito, se cobran USD 105). Registrar esa comisión en la compra de créditos y asignarla una sola vez al uso que financia; no añadirla también a cada request. Con **BYOK**, las credenciales del proveedor se facturan directamente por ese proveedor y no se aplica el 5 % de compra de créditos de Unified Billing. AI Gateway ofrece sin costo sus funciones básicas actuales (analítica, caché y rate limiting); almacenamiento/exportación de logs puede generar cargos separados según el producto de logs.

No aplicar la tarifa de neurons a los cuatro IDs terceros. Tampoco atribuir a cada tenant la cuota diaria de Workers AI, porque es una asignación global de la cuenta XOVYRA.

## 3. Impuestos y comisiones

- Mostrar antes de confirmar **subtotal, impuestos, comisión repercutida si es legal y contractualmente admisible, y total**. El tenant paga los impuestos aplicables en el checkout; XOVYRA conserva la responsabilidad de calcularlos, recaudarlos, declararlos y facturarlos correctamente. El IVA/impuesto de venta no es ingreso ni margen; impuestos sobre la renta se muestran en contabilidad y no se confunden con margen operativo.
- El motor fiscal aplica reglas versionadas por jurisdicción, producto, tipo de cliente y estado tributario. No asumir que todo servicio XOVYRA causa IVA del 19 %: es la tarifa general colombiana, pero la clasificación concreta del servicio debe validarse con asesoría tributaria antes de producción. No habilitar cobro hasta configurar dicha clasificación.
- Separar el IVA de la venta del IVA que Wompi cobra a XOVYRA sobre su comisión. El segundo es costo del procesador para XOVYRA, no el IVA del servicio al tenant.
- Las tasas publicadas por Wompi (Plan Avanzado: 2,65 % + COP 700 + IVA sobre la comisión) sirven como referencia inicial, no sustituyen el contrato ni se aplican a otros planes/métodos. El backend debe conciliar la liquidación real.
- Los impuestos no cuentan como ingreso ni como margen. Incluir el costo de procesamiento que corresponda en el precio comercial cuando no se pueda repercutir como línea separada.

## 4. Créditos, costo completo y motor de precios

- Los créditos son **unidades internas de consumo comercial**, no unidades de Cloudflare, dinero, saldo R2 ni una promesa de equivalencia fija con el costo del proveedor. El backend mantiene un parámetro `valor_referencia_credito` versionado para convertir el precio calculado a créditos. Valor de referencia inicial de diseño: **USD 0,01 por crédito**; puede cambiar solo para nuevas versiones de catálogo/operaciones, con aviso y sin recalcular movimientos históricos. COP usa equivalencia de catálogo versionada.
- Un modelo no tiene un precio fijo por modalidad. El catálogo versionado incluye proveedor e ID/versión real, unidades/medición oficiales, precio unitario fuente, parámetros que alteran consumo (resolución, duración, pasos, tokens u otros), moneda, fecha de vigencia, multiplicador de seguridad configurable, costo operativo asignado, comisión aplicable, margen objetivo, redondeo y créditos a reservar/consumir.
- Para cada ejecución calcular `C` = costo medido/estimado del proveedor, `O` = costos operativos asignados al uso (infraestructura, procesamiento local/scratch, almacenamiento temporal, soporte/operación, pérdidas/fraude cuando exista una política de asignación), `F` = parte atribuida de cargos fijos de cobro, `r` = comisión porcentual efectiva del método, `m` = objetivo de margen operativo configurado. Precio antes de impuestos: `P = (C + O + F) / (1 - r - m)`. Si no existe cargo porcentual, `r=0`; si el denominador es menor o igual a cero, rechazar la tarifa. El multiplicador de seguridad se aplica al costo incierto antes de evaluar el precio, queda registrado y no sustituye reconciliación.
- **No llamar “margen neto” a la meta automática.** La fórmula permite apuntar a margen de contribución/operativo antes del impuesto a la renta solo en la medida en que `O` incluya los costos que finanzas decide asignar. Reportar por separado margen bruto de inferencia, contribución tras procesamiento, margen operativo tras gastos asignados y resultado neto contable. El neto final depende de costos fijos no asignados, impuestos a la renta y contabilidad externa; no se garantiza con una tarifa por generación.
- Convertir a créditos con `créditos = ceil(P / valor_referencia_credito)`, redondeando hacia arriba según la precisión configurada. Mostrar el precio/consumo estimado y créditos antes de confirmar; el cliente ve créditos y totales monetarios necesarios en checkout/factura, pero nunca se presenta el crédito como costo de Cloudflare.
- El ledger monetario inmutable conserva saldo, reservas y asientos; cada movimiento identifica origen (plan/paquete/ajuste), tenant, vencimiento, catálogo, precio, costo asignado y versión. Evitar saldo negativo mediante bloqueo/concurrencia transaccional. Consumir primero lotes próximos a vencer (FIFO por vencimiento); devolver saldo con asientos reversos, nunca borrando historia.
- Antes de ejecución, reservar créditos calculados con precio vigente y una estimación/multiplicador prudente; liquidar con unidades/costo real y liberar excedente. Fallos sin costo liberan reserva; intentos con costo real quedan individualizados. Idempotencia por operación evita cargos dobles y un retry solo genera cargos por costo real nuevo. Mantener límite máximo aceptado para no cargar diferencias no autorizadas.
- Procesamiento de pago ocurre en la venta de una suscripción/paquete, no en cada generación. Registrar la tarifa PSP una sola vez por transacción. Para análisis de rentabilidad, asignar cargos fijos de invoice al plan según la política de distribución configurada (por ingresos/créditos emitidos/uso esperado) y cargo fijo de paquete a los créditos/uso estimado del paquete. No cargar una comisión fija completa a cada imagen ni contar dos veces `r/F` entre precio unitario y cobro del producto.
- Evaluar cada plan antes de publicarlo con escenarios de consumo bajo, esperado y alto, mezcla real de modelos, créditos acumulables/expirados, comisión PSP, impuestos no recuperables, reembolsos/contracargos, costos `O`, concurrencia y límites. Alertar/bloquear publicación si cae por debajo del margen operativo mínimo aprobado. La suscripción no garantiza que cada crédito se use ni debe asumir que el consumo será siempre el promedio; configurar cuotas y límites de concurrencia/gasto por tenant/modalidad.
- Costos y tarifas inciertos se versionan. El cálculo se apoya en unidades oficiales por modelo; no aplica la tarifa Workers AI en neurons a modelos de terceros. Para estos modelos, conciliar por separado los créditos/cargos de Unified Billing o la factura del proveedor en BYOK; los créditos o beneficios globales de cuenta no reducen el costo unitario usado para fijar precios al cliente.

## 5. Catálogo inicial propuesto, sujeto a simulación

Este catálogo es una hipótesis de lanzamiento, no una garantía de margen ni un precio ya validado. Moneda antes de impuestos; el procesamiento se muestra aparte solo cuando lo permitan ley/contrato y, si no, queda incorporado al precio público. El valor de cada crédito se usa como referencia de consumo versionada, no como valor del costo de proveedor.

| Plan mensual | Precio USD de referencia | Créditos iniciales | Perfil |
|---|---:|---:|---|
| Básico | 9 | 750 | Una persona, límites base |
| Pro | 19 | 1.580 | Creador frecuente, límites ampliados |
| Business | 49 | 4.080 | Equipo, colaboración y cuotas ampliadas |

| Paquete adicional | Precio USD de referencia | Créditos iniciales |
|---|---:|---:|
| Mini | 5 | 400 |
| Plus | 15 | 1.250 |
| Studio | 50 | 4.200 |

Los precios y créditos iniciales se usan solo después de ejecutar la simulación del apartado 4 con tarifas Cloudflare verificadas y la cuenta Wompi/Stripe real. Si la simulación no alcanza la meta aprobada, ajusta precio/créditos, costos asignados, capacidades o límites antes de publicarlos. No fijar aún precios COP por conversión directa: Finanzas mantiene catálogo local versionado y redondeado. No ofrecer créditos extra de regalo/descuentos sin volver a simular márgenes.

La cuota fija también financia disponibilidad y costos generales cubiertos por la suscripción; deben aparecer como partidas diferentes al costo de inferencia en reportes internos. Las cuotas, concurrencia, retención, usuarios y capacidades incluidas se fijan por versión del plan. No prometer uso ilimitado.

## 6. Suscripciones y ciclo de cobro

- Cobro mensual anticipado. Otorgar créditos del periodo únicamente tras confirmación firmada/idempotente del pago o factura pagada; nunca por crear una intención de pago.
- Renovación fallida: reintentos programados a los días 1, 3 y 7; estado `past_due` y gracia de 7 días. Durante gracia no otorgar nuevos créditos ni ampliar límites; conservar proyectos y permitir exportar/descargar recursos conforme a la política de almacenamiento. Al terminar, suspender generación y funciones de suscripción, sin borrar inmediatamente datos.
- Cancelación efectiva al final del periodo pagado. El saldo de créditos mensuales vence a los 60 días desde su emisión y puede pasar como máximo a un ciclo posterior; créditos adicionales comprados no vencen mientras la cuenta siga activa, sujeto a ley. Mostrar expiración y consumir por FIFO de vencimiento.
- Cambios de plan se prorratean solo si la integración y la normativa configurada lo soportan; de otro modo, se programan para el siguiente ciclo. Webhooks duplicados o fuera de orden no duplican cobros, grants ni facturas.
- La recurrencia usa fuentes tokenizadas/mandatos del PSP. Nunca almacenar PAN/CVV ni procesar secretos de tarjeta en XOVYRA. El estado local se reconcilia con PSP y factura.

## 7. R2 opcional: saldo monetario independiente

- El almacenamiento persistente es opt-in explícito y se financia con saldo monetario separado; ni créditos de IA ni créditos mensuales se convierten en saldo R2. Tarifa según GB-mes, operaciones billables y retrieval cuando aplique, atribuida al tenant/propietario; aplicar la fórmula de margen anterior con tarifas R2 versionadas.
- Proponer recarga manual mínima equivalente a USD 5 para reducir el impacto de comisiones fijas. El saldo acreditado coincide con el valor comprado; la comisión/impuesto del pago se informa y se cobra adicionalmente cuando esté permitido, o se incorpora al precio de recarga publicado, sin descontarlo ocultamente del saldo R2. Recarga automática desactivada inicialmente; si se habilita luego, requiere consentimiento separado, umbral, monto y tope mensual definidos por el usuario.
- Al aceptar persistencia, comprobar saldo suficiente y reservar el costo proyectado de 30 días. Medir diariamente, liquidar uso real y liberar reserva sobrante. Avisar al llegar al 20 % del saldo o a siete días de consumo estimado. Bloquear nuevas cargas y guardados permanentes si no hay fondos suficientes; permitir exportar/descargar lo ya almacenado.
- Saldo agotado: gracia de pago de 7 días con avisos; luego 30 días en modo solo lectura/exportación antes de programar borrado irreversible. Enviar avisos repetidos y mostrar la fecha límite. Un pago válido detiene el borrado. La cancelación voluntaria aplica el mismo periodo de exportación de 30 días. Un fallo de suscripción, por sí solo, no elimina recursos R2 prepagados.
- Sin persistencia contratada, resultados temporales se conservan 24 horas y luego se eliminan automáticamente; mostrar el vencimiento antes de generar/guardar. Este uso temporal necesario está incluido como costo de operación, no crea almacenamiento permanente.
- Antes del borrado, comprobar referencias/retenciones legales, emitir evento auditable y hacer la eliminación irreversible solo al vencer la ventana. La restauración posterior no se garantiza.

## 8. Proveedores y activación

- Wompi es el procesador COP inicial. Integrar sandbox, validación de firma/webhook, idempotencia, conciliación y estados antes de activar producción.
- Stripe será el procesador USD internacional usando la cuenta existente que confirma el propietario, previa validación del estado merchant, jurisdicción, métodos, monedas, tarifas y webhooks. No guardar secretos en el frontend ni en documentación; mantener credenciales en el gestor de secretos del backend.
- Contratos de proveedores, tarifas, precio, impuestos, moneda, margen, política de expiración/retención y consentimientos se versionan con vigencia, auditoría y referencia. No modificar asientos históricos.

## 9. Bloqueos previos a cobro real

1. Asesor tributario valida clasificación IVA de SaaS/IA/almacenamiento, tratamiento de créditos prepagados, facturación electrónica, devoluciones y ventas internacionales.
2. XOVYRA obtiene y registra tarifas/contrato vigente de Wompi, medio de pago, cuenta de liquidación y credenciales productivas.
3. Para Stripe USD, validar la cuenta existente, su estado merchant, país, liquidación, monedas y credenciales; configurar el fee schedule y probar el sandbox antes de activar live.
4. Finanzas configura catálogo COP/USD inicial, referencia de crédito, FX de catálogo, reglas fiscales/redondeo, asignación de costos operativos y simulaciones aprobadas de planes/límites.
5. Cloudflare verifica tarifas del catálogo seleccionado/R2, modo Unified Billing o BYOK, unidades por modelo y conciliación con facturas de la cuenta.

## Fuentes oficiales consultadas

- [Tarifas Wompi Colombia](https://wompi.co/es/co/planes-tarifas/) y [centro de ayuda de tarifas](https://soporte.wompi.co/hc/es-419/articles/360020957133--Cu%C3%A1les-son-los-planes-y-tarifas-que-maneja-la-plataforma-Wompi).
- [Fuentes tokenizadas y recurrentes de Wompi](https://docs.wompi.co/docs/colombia/fuentes-de-pago/), [impuestos Wompi](https://docs.wompi.co/docs/colombia/impuestos/) e [IVA sobre comisión](https://soporte.wompi.co/hc/es-419/articles/1500009288841--Qu%C3%A9-impuesto-me-cobra-la-Pasarela-Wompi-y-cu%C3%A1l-es-el-porcentaje).
- [Tarifas Stripe por país, tarjeta internacional y conversión](https://stripe.com/pricing), [monedas admitidas](https://docs.stripe.com/currencies) y [precios recurrentes](https://docs.stripe.com/api/prices).
- [Tarifas de cada modelo en Cloudflare AI](https://developers.cloudflare.com/ai/models/black-forest-labs/flux-3-image-klein/), [P-Image](https://developers.cloudflare.com/ai/models/pruna/p-image/), [P-Video](https://developers.cloudflare.com/ai/models/pruna/p-video/) y [MiniMax H3 Max](https://developers.cloudflare.com/ai/models/minimax/h3-max/).
- [Precios AI Gateway / Unified Billing](https://developers.cloudflare.com/ai-gateway/reference/pricing/) y [precios de Workers AI neurons](https://developers.cloudflare.com/workers-ai/platform/pricing/) (solo para modelos `@cf/`).
- [Workers AI FLUX.2 Klein 4B](https://developers.cloudflare.com/workers-ai/models/flux-2-klein-4b/), [Klein 9B](https://developers.cloudflare.com/workers-ai/models/flux-2-klein-9b/), [FLUX.2 Dev](https://developers.cloudflare.com/workers-ai/models/flux-2-dev/), [Lucid Origin](https://developers.cloudflare.com/workers-ai/models/lucid-origin/), [Phoenix 1.0](https://developers.cloudflare.com/workers-ai/models/phoenix-1.0/), [FLUX.1 Schnell](https://developers.cloudflare.com/workers-ai/models/flux-1-schnell/), [SDXL-Lightning](https://developers.cloudflare.com/workers-ai/models/stable-diffusion-xl-lightning/), [DreamShaper 8 LCM](https://developers.cloudflare.com/workers-ai/models/dreamshaper-8-lcm/), [Stable Diffusion v1.5 Inpainting](https://developers.cloudflare.com/workers-ai/models/stable-diffusion-v1-5-inpainting/) y [SDXL Base 1.0](https://developers.cloudflare.com/workers-ai/models/stable-diffusion-xl-base-1.0/).
- [API de envío e idempotencia de Resend](https://resend.com/docs/api-reference/emails/send-email), [dominios y verificación de remitente](https://resend.com/docs/dashboard/domains/introduction) y [webhooks de Resend](https://resend.com/docs/dashboard/webhooks/introduction).
- [Tarifas R2](https://developers.cloudflare.com/r2/pricing/).
- [DIAN, Oficio 24380 de 2019 sobre servicios en nube](https://normograma.dian.gov.co/dian/compilacion/docs/oficio_dian_24380_2019.htm) y [documento DIAN 2026 sobre base gravable y tarifa general](https://www.dian.gov.co/atencionciudadano/contactenos/Annimos%20PQRS/2026DP000061655.pdf).
