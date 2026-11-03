# Configuração de Distância Mínima via Linha de Comando

## Como usar o novo argumento `-m` / `--min-separation-mm`

### Exemplos de uso:

1. **Distância mínima de 3mm:**
```bash
cargo run -- -i data/input/albano.json -t 60 -m 3.0
```

2. **Distância mínima de 5mm com DPI personalizado (pontos por polegada):**
```bash
cargo run -- -i data/input/albano.json -t 60 -m 5.0 --dpi 300
```

3. **Sem distância mínima (comportamento padrão):**
```bash
cargo run -- -i data/input/albano.json -t 60
```

### Parâmetros:

- **`-m, --min-separation-mm <VALOR>`**: Define a distância mínima em milímetros
- **`--dpi <VALOR>`**: DPI real, em pontos por polegada (padrão: 96)

### Conversão automática:

```
unidades_internas = milímetros × dpi / 25.4
```

Com o padrão de 96 DPI, `-m 3.0` vale `3 × 96 / 25.4 ≈ 11.3386` unidades de entrada.
Se o JSON já traz `min_item_separation`, esse valor é usado. `-m` substitui o campo
somente quando a flag é passada. Sem `-m` e sem o campo, a separação é zero.

O padrão antigo `--dpi 3.7795275591` já era pixels por milímetro (`96 / 25.4`) e
ainda era dividido por 25.4, então 3 mm viravam ~0.4464 em vez de ~11.34. Não foi
possível confirmar a linha de comando do TC.ThaiAPI. O padrão agora corrige esse
cálculo. Para reproduzir o número antigo, exporte `SPARROW_LEGACY_DPI=1` (o padrão
volta a ser `3.7795275591`, na mesma fórmula). Um `--dpi` explícito continua sendo
DPI de verdade, com ou sem essa variável.

### Log de confirmação:

Quando você usar `-m 3.0`, verá no log:
```
[INFO] [00:00:00] <main> minimum separation: 3.000mm = 11.338583 input units (DPI: 96.000000)
```

### Vantagens:

1. **Interface amigável**: Trabalhe diretamente em milímetros
2. **Flexibilidade**: Pode ajustar o DPI se necessário
3. **Compatibilidade**: Funciona junto com todos os outros argumentos existentes
4. **Validação**: O sistema mostra exatamente os valores convertidos
