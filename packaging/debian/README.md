# Paquete Debian de Óxido

El empaquetado se implementa por subfases. Esta primera subfase solo comprueba la toolchain disponible: exige `rustc` y `cargo` estables 1.99.1 o posteriores, y si `rustfmt` existe verifica que corresponda al mismo commit que `rustc`.

El preflight no instala, actualiza ni desinstala Rust. Si no hay toolchain, informa que debe provisionarse en la subfase correspondiente. Una versión anterior o una combinación incoherente produce un error que identifica el requisito; el instalador nunca debe reemplazar la toolchain del usuario.

Ejecutar la regresión:

```sh
bash packaging/debian/tests/check-rust-toolchain.sh
```

El script de verificación se puede invocar directamente:

```sh
bash packaging/debian/check-rust-toolchain.sh
```

Todavía no se genera un `.deb` ni se configura una toolchain aislada. La política de mantenedores Debian no debe descargar ni instalar toolchains: la instalación y actualización pertenecen a un paso explícito y reversible del instalador, fuera de los scripts `preinst`/`postinst`.
