# EarthCraft — camadas adicionadas em outubro de 2026

## Terra plana 3D (easter egg)

Ativação: **Ctrl+Shift+F** (macOS: ⌘+Shift+F) ou procurar **terra plana** / **flat earth**.
A cena de humor agora apresenta céu estrelado, disco 3D texturizado e borda de gelo com
espessura e controles de perspectiva. Arraste para girar em 3D; segure Shift e arraste
para deslocar; rode o mouse para zoom; duplo clique para restaurar; Esc para sair.
É um desenho fictício, separado de todos os mapas e sistemas de coordenadas reais.

## Aviões

`apps/earthcraft/assets/airplane.svg` é o novo desenho SVG da aeronave.
O PNG derivado `airplane.png` é incorporado para rasterização eficiente no egui;
os ícones são orientados de acordo com o rumo do ADS-B/OpenSky. Mantém os
provedores e avisos anteriores.

## Fronteiras de cidades no mundo

O pacote mundial Natural Earth continua fornecendo nomes de cidades e
fronteiras nacionais/estaduais. Para **limites municipais detalhados**, ao ativar
**Limites de cidades/municípios** e aproximar a câmera até **110 km**, o app consulta
uma área de cerca de 0,6° ao redor da câmera usando limites administrativos
nível 8 ou 9 do **OpenStreetMap via Overpass**. A resposta substitui somente o
cache regional OSM, sem acumular todas as cidades do planeta em RAM.
O carregamento é limitado a no máximo uma tentativa a cada 75 segundos e
refaz a área quando o usuário se desloca. O cache de limites oficiais
municipais brasileiros do IBGE permanece como antes. Algumas áreas/países
não têm contornos completos no OSM; a aplicação não inventa fronteiras.

## Navios AIS (agora mundiais)

Ative **Navios AIS mundiais**. Por padrão, a camada consulta a fotografia
consolidada da frota no **OpenSeaFeed** (`https://api.openseafeed.com/v1/snapshot`),
sem chave de API, a cada 120 segundos. Trata-se de dados AIS coletados por
estações e fontes participantes ao redor do planeta, com posições, identificador
MMSI, nome, rumo, velocidade e horário da observação. A cobertura é **mundial,
mas não completa**: alto-mar, navios que não transmitem e regiões sem receptores
ficam sem registros. O filtro descarta posições com mais de 90 minutos. Não
há animação de trajetos inventados nem indicação de localização exata atual
quando a observação estiver atrasada.

Se o OpenSeaFeed estiver indisponível, há contingência via **Fintraffic**:
nesse caso a interface informa expressamente que os navios mostrados estão
restritos às águas da Finlândia. Para substituir a fonte por serviço contratado,
permanece disponível `EARTHCRAFT_AIS_GEOJSON_URL` com um GeoJSON FeatureCollection
HTTPS compatível. Respeite a licença e a atribuição de dados do OpenSeaFeed.

## Satélites

Ative **Satélites (órbitas CelesTrak, estimadas)**. Elementos orbitais OMM/JSON
dos grupos **STATIONS**, **GPS-OPS** e **WEATHER** são consultados no máximo a cada
seis horas. Os pontos são calculados com o modelo de Kepler de dois corpos
(aproximação educacional), atualizados em tela. Eles **não** são telemetria ao
vivo, nem fornecem precisão SGP4. É importante não usar para observações
científicas, apontamento de antenas ou controle operacional. Recusa elementos
com mais de 14 dias de defasagem temporal.

## Estradas do mundo, sem chave de API

Ative **Estradas do mundo (OpenStreetMap)**. O EarthCraft consulta os traçados
rodoviários perto da câmera em qualquer país através de instâncias públicas do
**Overpass**, sem cadastro ou chave de API (o Overpass é tecnicamente uma API
pública de leitura). A busca é limitada à região observada, só quando a câmera
está abaixo de 180 km e com intervalos de atualização para respeitar os servidores.
Passe o mouse sobre uma via para ver o nome e a classificação. Em cinza as vias
têm **fluxo desconhecido**; em vermelho aparecem apenas as vias marcadas no
OSM com construção/restrição de acesso. **Vermelho aqui não significa trânsito
parado**: dados OSM podem estar desatualizados, e ausência de restrição também
**não** significa tráfego livre. Crédito: © OpenStreetMap contributors (ODbL).

## Congestionamento rodoviário de verdade (opcional)

A camada **Congestionamento real (TomTom)** anterior permanece disponível, com
verde = livre, amarelo = lento, vermelho = congestionado, **somente** quando
uma chave TomTom autorizada é fornecida. Ela pode ser sobreposta às estradas OSM.
Sem observações externas de velocidades não é tecnicamente possível determinar
em tempo real se determinada avenida está parada, mesmo conhecendo seu traçado.
O EarthCraft não fabrica congestionamentos nem declara vias livres sem evidências.

## Referências

- OpenStreetMap / Overpass — https://wiki.openstreetmap.org/wiki/Overpass_API
- OpenSeaFeed — https://openseafeed.com/
- Digitraffic/Fintraffic (contingência) — https://www.digitraffic.fi/en/marine-traffic/
- CelesTrak — https://celestrak.org/NORAD/documentation/gp-data-formats.php
- TomTom Orbis v2 — https://docs.tomtom.com/traffic-api/documentation/tomtom-orbis-maps/v2/traffic-flow/raster-flow-tiles

## Verificação

Os testes unitários foram adicionados aos módulos de geometria, AIS mundial, vias OSM, órbitas,
trânsito e OSM. **Não é possível declarar compilação ou execução desses testes
sem uma instalação de Rust 1.95 ou mais recente**. No repositório, execute
`cargo fmt --all`, `cargo test --workspace --locked` e `cargo build --workspace --locked`
e confira todas as camadas em uma máquina com acesso à internet. Os serviços
externos não foram consultados a partir do ambiente de empacotamento.
