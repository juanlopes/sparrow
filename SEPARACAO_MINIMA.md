# Configuração de Distância Mínima via Linha de Comando

## Como usar o novo argumento `-m` / `--min-separation-mm`

### Exemplos de uso:

1. **Distância mínima de 3mm:**
```bash
cargo run -- -i data/input/albano.json -t 60 -m 3.0
```

2. **Distância mínima de 5mm com DPI personalizado:**
```bash
cargo run -- -i data/input/albano.json -t 60 -m 5.0 --dpi 4.0
```

3. **Sem distância mínima (comportamento padrão):**
```bash
cargo run -- -i data/input/albano.json -t 60
```

### Parâmetros:

- **`-m, --min-separation-mm <VALOR>`**: Define a distância mínima em milímetros
- **`--dpi <VALOR>`**: Define o DPI para conversão (padrão: 3.7795275591)

### Conversão automática:

O sistema converte automaticamente milímetros para unidades internas usando a fórmula:
```
unidades_internas = milímetros × DPI ÷ 25.4
```

### Log de confirmação:

Quando você usar `-m 3.0`, verá no log:
```
[INFO] [00:00:00] <main> minimum separation: 3.000mm = 0.446400 internal units (DPI: 3.779528)
```

### Vantagens:

1. **Interface amigável**: Trabalhe diretamente em milímetros
2. **Flexibilidade**: Pode ajustar o DPI se necessário
3. **Compatibilidade**: Funciona junto com todos os outros argumentos existentes
4. **Validação**: O sistema mostra exatamente os valores convertidos
