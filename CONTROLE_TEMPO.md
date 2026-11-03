# Guia Completo: Controle de Tempo de Execução

## ✅ Sistema Já Implementado!

O `sparrow` já possui um sistema robusto de controle de tempo que **automaticamente retorna o melhor resultado** encontrado dentro do limite especificado.

## 🕐 Opções de Controle de Tempo

### 1. **Tempo Global** (Recomendado para a maioria dos casos)
```bash
# Executa por 60 segundos total
cargo run -- -i data/input/albano.json -t 60

# Executa por 300 segundos (5 minutos) total
cargo run -- -i data/input/albano.json -t 300

# Com separação mínima de 3mm por 120 segundos
cargo run -- -i data/input/albano.json -t 120 -m 3.0
```

**Como funciona:**
- 80% do tempo é usado para **exploração** (encontrar soluções viáveis)
- 20% do tempo é usado para **compressão** (otimizar a melhor solução)
- O limite é rígido: a soma das duas fases não passa de `-t`. Ao atingir o prazo
  a busca para, inclusive no meio de uma iteração de separação, e o melhor
  layout completo encontrado até ali é gravado em `output/final_{nome}.json` e
  `.svg`. A construção inicial (para ter uma solução completa) roda antes da
  busca; nas instâncias pequenas ela é imediata.

### 2. **Controle Detalhado** (Para usuários avançados)
```bash
# 180s exploração + 60s compressão = 240s total
cargo run -- -i data/input/albano.json -e 180 -c 60

# 300s exploração + 120s compressão = 420s total
cargo run -- -i data/input/albano.json -e 300 -c 120 -m 2.5
```

### 3. **Padrão** (Se não especificar tempo)
```bash
# Usa 600 segundos (10 minutos) por padrão
cargo run -- -i data/input/albano.json -m 3.0
```

## 🎯 Como o Sistema Garante o Melhor Resultado

### Durante a Execução:
1. **Fase de Exploração**: Busca soluções viáveis reduzindo gradualmente a largura
2. **Fase de Compressão**: Otimiza intensivamente a melhor solução encontrada
3. **Terminação Automática**: Para automaticamente quando o tempo limite é atingido
4. **Melhor Resultado**: Retorna sempre a melhor solução encontrada até o momento

### Logs de Progresso:
```
[INFO] [00:00:15] <main> [EXPL] feasible solution found! (width: 2456.789, dens: 87.3%)
[INFO] [00:01:23] <main> [EXPL] feasible solution found! (width: 2234.456, dens: 91.2%)
[INFO] [00:02:45] <main> [CMPR] success at 99.5% (2198.123 | 92.8%)
```

## 📊 Saídas Geradas

### Arquivos Automáticos:
- **`output/final_{nome}.svg`**: Visualização da melhor solução
- **`output/final_{nome}.json`**: Dados completos da solução
- **`output/log.txt`**: Log detalhado da execução

### Informações da Solução:
```json
{
  "solution": {
    "strip_width": 2198.123,
    "density": 0.928,
    "items": [...]
  }
}
```

## ⚡ Exemplos Práticos

### Execução Rápida (1 minuto):
```bash
cargo run -- -i data/input/albano.json -t 60 -m 3.0
```

### Execução Média (5 minutos):
```bash
cargo run -- -i data/input/albano.json -t 300 -m 2.0
```

### Execução Longa (20 minutos):
```bash
cargo run -- -i data/input/albano.json -t 1200 -m 1.5
```

### Controle Fino:
```bash
# 15 min exploração + 5 min compressão
cargo run -- -i data/input/albano.json -e 900 -c 300 -m 2.5
```

## 🔄 Terminação Prematura

### Ctrl+C (Terminação Manual):
- O sistema captura Ctrl+C e **salva a melhor solução** encontrada até o momento
- Não há perda de progresso!

### Terminação Automática:
- O algoritmo pode terminar antes do limite se encontrar a solução ótima
- Sempre retorna o melhor resultado disponível

## 📈 Dicas de Desempenho

### Para Resultados Rápidos:
```bash
cargo run -- -i data/input/albano.json -t 30 -m 3.0
```

### Para Máxima Qualidade:
```bash
cargo run -- -i data/input/albano.json -t 3600 -m 1.0  # 1 hora
```

### Balanceamento Personalizado:
```bash
cargo run -- -i data/input/albano.json -e 1800 -c 1200 -m 2.0  # 50 min total
```

## 🎯 Resumo

✅ **Sistema completo de tempo já implementado**  
✅ **Retorna automaticamente o melhor resultado**  
✅ **Terminação segura e controlada**  
✅ **Múltiplas opções de configuração**  
✅ **Logs detalhados de progresso**  
✅ **Saídas em SVG e JSON**

**Use `-t SEGUNDOS` para controle simples e efetivo!**
