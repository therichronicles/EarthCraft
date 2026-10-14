# Arquitetura

[English](ARCHITECTURE.md) | [Português (Brasil)](ARCHITECTURE.pt-BR.md)

EarthCraft 0.1.0 usa um workspace Cargo com uma aplicação e três bibliotecas.

| Pacote | Responsabilidade |
| --- | --- |
| `apps/earthcraft` | Inicialização, entrada, estado da interface, fontes, formas do egui e upload de texturas |
| `earthcraft-core` | Matemática/câmera em f64, seleção, animação de voo, normalização Unicode, idiomas, Fluent e marcadores |
| `earthcraft-data` | HTTP assíncrono, tiles, limites administrativos, busca, KML, configurações e persistência |
| `earthcraft-render` | Rasterização CPU, coordenação de quadros, projeção de overlays, índices espaciais e prioridade de tiles |

A aplicação depende das bibliotecas; render depende de core/data e data depende de
core. As bibliotecas nunca dependem da aplicação. Versões e perfis ficam no
manifesto do workspace; `Cargo.lock` fixa as dependências. `cargo run --release`
inicia a aplicação a partir da raiz.

## Renderização

Uma thread de coordenação mantém um trabalho ativo, um pedido pendente substituível
e um quadro concluído. Mudanças de provedor, versão histórica, tamanho ou estilo
rejeitam resultados incompatíveis. Seleção, overlays e escala usam a câmera do
quadro exibido, mantendo alinhamento durante movimento.

A CPU usa Rayon, blocos de oito linhas e quatro raios f64 por lote com `wide`.
Glam fornece matrizes/quaternions e cores SIMD; bytemuck fornece conversões seguras
de pixels e uniforms. O workspace proíbe Rust inseguro. Os menus nativos do
macOS ficam no desktop; seus callbacks enfileiram ações e solicitam uma
atualização do egui. Windows e Linux usam os controles do painel lateral.
O botão na barra de ferramentas permite ocultar e reabrir o painel lateral,
preservando seus controles e seções. Buffers são reutilizados
quando exclusivos; capturas retêm `Arc` imutável e são codificadas na thread de
arquivos. O egui continua responsável pelo upload e desenho das texturas.

## Overlays e serviços

Render prepara visibilidade, projeção de linhas e candidatos de rótulos fora do
egui. `geo-index` indexa linhas imutáveis com testes conservadores de horizonte e
frustum, incluindo polos e antimeridiano. `rstar` controla colisões de retângulos
na tela, preservando prioridade, contato nas bordas e limite de 500 rótulos.
A aplicação mede fontes e mantém um cache de formas para a vista exibida; câmera,
viewport, DPI, dados, camadas, idioma e escala de texto invalidam esse cache.
Marcadores e régua permanecem interativos.

Um `ServiceRuntime` Tokio fornece I/O assíncrono e clientes reqwest reutilizados.
Workers limitados separam parsing de dados, decodificação de tiles e arquivos.
GeoJSON é processado por feature, com coordenadas compartilhadas e snapshots
imutáveis. Geometria municipal carrega quando suas camadas são usadas. Busca
local usa índices FST com normalização Unicode e fallback por varredura durante
reconstrução. GeoRust `kml` lê/escreve XML após uma preparação limitada com
quick-xml. Diálogos e aplicação dos resultados pertencem à aplicação; KML, PNG e
persistência de preferências ficam em workers. Preferências são gravadas de forma
coalescida e descarregadas ao encerrar.

O Explorer também usa o serviço de arquivos para decodificar imagens, inclusive
a Lua incorporada, e ler/gravar documentos de planetas. O upload de texturas
permanece no egui. As imagens originais ficam compartilhadas em memória para
permitir salvar novamente um planeta importado sem depender de caminhos antigos
ou recodificar JPEG/PNG. A gravação captura o nome e as fontes ao ser solicitada.

Aeronaves reais usam um controlador assíncrono em data, com consultas de câmera
coalescidas e snapshots imutáveis compartilhados. Decodificação, união das fontes
e ordenação ficam no pool limitado de features. Render prepara a projeção e os
candidatos de rótulos fora do egui, junto ao quadro durante movimento e em um
worker limitado quando parado. A aplicação reutiliza malhas dos ícones e galleys
do quadro atual; mantém fontes, hover, Fluent e diagnósticos em inglês. Resultados
com outra câmera, escala, DPI ou conjunto de dados não são desenhados. Novas
consultas pausam quando a camada ou a Terra estão ocultas. Mantêm-se os intervalos
das fontes e o limite de extrapolação de 25 segundos.

## Caches e contratos

- Moka mantém tiles decodificados com LRU e meta aproximada de 256 MiB; referências
  temporárias e ultrapassagens breves são contabilizadas separadamente.
- MBTiles 1.3 usa tokio-rusqlite e meta de 128 MiB de arquivos gerenciados, incluindo
  overhead SQLite. Provedores, versões Wayback e formatos ficam separados; XYZ é
  convertido para TMS. Arquivos antigos são importados sob demanda.

A limpeza remove apenas imagens gerenciadas, bloqueia gravações anteriores ao
pedido, preserva tiles em RAM/configurações/limites administrativos e atualiza o
uso em disco. Aquecimento global de satélite cobre zooms 0–4, 341 tiles e 85,25 MiB
decodificados, enquanto o app está ocioso a 14.000 km ou mais. Pedidos visíveis têm
prioridade; outros provedores permanecem sob demanda.

O cache usa `LOCALAPPDATA`, `XDG_CACHE_HOME`, `HOME/.cache` ou diretório temporário,
sob `earthcraft_tiles`. Preferências ficam em `settings.json` e `lang.txt`.
Coordenadas usam graus; altitude/distâncias usam quilômetros. KML/GeoJSON usam
longitude primeiro; pares de domínio usam latitude primeiro. Fluent mantém
catálogos en/pt-BR com chaves semânticas e argumentos nomeados. Erros e logs usam
inglês; mensagens normais seguem o idioma escolhido.

Veja [CONTRIBUTING.md](CONTRIBUTING.md), [LOCALIZATION.md](LOCALIZATION.md) e
[GEOSPATIAL.md](GEOSPATIAL.md) para práticas de manutenção e validação. Profiling é
opcional e usa um gravador limitado em segundo plano; tempos de componentes não
representam automaticamente FPS da aplicação.


## Experimental GPU Earth rasterization / renderização experimental em GPU

A interface usa o backend wgpu padrão do eframe, com os fallbacks gráficos
originais habilitados, independentemente do cálculo do globo na CPU ou GPU.
A preferência de FPS sem limite altera o modo de apresentação na inicialização.

The Earth overview raster can be computed by the `earthcraft-render` `backend`
module with `wgpu` compute shaders (`gpu/`), then read back for the existing
egui view. All GPU globe rendering is experimental on every platform; the CPU
path remains the default and fallback. Set
`EARTHCRAFT_RENDERER=auto` or `EARTHCRAFT_RENDERER=gpu` to enable an experimental GPU attempt.
The Explorer, live meteorological/aircraft layers, moon and custom planet
remain integrated in `apps/earthcraft`. See [GPU_RENDERER.md](GPU_RENDERER.md)
for hardware limits, performance and CPU-only build instructions.

## Limites entre frontends para o EarthCraft web

Core contém a seleção de corpos celestes, o cálculo esférico de destinos e os
cálculos de órbita e zoom. Render contém malhas dos corpos (posições, UVs e
índices), composição meteorológica, projeção de aeronaves e régua, escala,
resolução e chaves de compatibilidade dos quadros. Essas APIs usam valores de
domínio, imagens e `emath`, sem widgets egui, texturas ou janelas. O adaptador
desktop envia texturas, mede fontes, recebe entradas e desenha a geometria.

Data contém downloads de nuvens/radar, atualização e novas tentativas, conversão
da paleta, URLs geográficas e documentos `.earthplanet`. As APIs de codificação
e leitura de bytes permitem ao frontend resolver imagens externas da versão
antiga; a leitura de caminhos nativos fica separada. Abrir URLs, sintetizar voz
e mostrar diálogos continuam sendo responsabilidades do desktop.

Essa extração prepara a lógica compartilhada; não torna Tokio, SQLite, serviços
de arquivos nativos ou leitura de GPU compatíveis com WebAssembly. O frontend web
ainda precisa de adaptadores de rede, armazenamento e execução do navegador.
A projeção meteorológica e a construção das malhas mantêm seu agendamento atual;
essa mudança não adiciona tarefas em segundo plano.

O core oferece modelos da Terra separados da geometria: Realista — WGS 84 (G2296) por padrão e Esfera perfeita como alternativa. Câmera, seleção, rasterização, sobreposições e geodésicas usam o modelo selecionado. A troca invalida quadros/caches incompatíveis; o elipsoide usa CPU. Consulte [Earth models](EARTH_MODELS.md) para limites de referencial/época e extensão.

A árvore de lugares pertence ao core e preserva Documentos/Pastas KML, ordem, nomes, descrições, pastas vazias e visibilidade herdada. A interface permite seleção, edição e movimentos sem ciclos; a leitura e gravação continuam no serviço de arquivos. Geometrias não suportadas mantêm a política de importar apenas a primeira coordenada.
Os caminhos dos arquivos KML abertos ficam separados das preferências em `open-kml.json`, sem copiar o conteúdo. Na inicialização, o serviço de arquivos reabre as fontes em ordem e remove silenciosamente os caminhos ausentes; outros erros são exibidos e os caminhos ficam para a próxima tentativa. Salvar a árvore inteira substitui a lista pelo destino salvo para evitar importações duplicadas. Limpar tudo esvazia a árvore e a lista e descarta resultados KML pendentes, sem excluir os arquivos de origem. Alterações não salvas ainda exigem uma gravação explícita do KML.

Arquivos KMZ são lidos e gravados no mesmo serviço de arquivos do KML. A importação usa `KmlReader::from_kmz_path` do GeoRust, selecionando o primeiro KML em ordem de arquivo sem extrair recursos para disco. Erros são exibidos diretamente; o KMZ não passa pela preparação XML dos arquivos KML simples. Visibilidade de pastas e tratamento de documentos malformados seguem o leitor upstream. A exportação grava a árvore suportada em `doc.kml` comprimido, usando a mesma versão da dependência ZIP do GeoRust. Recursos incorporados, estilos e links de rede mantêm as limitações do importador.
