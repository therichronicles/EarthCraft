# Embedded UI catalog. Keep keys and variables in sync across locales.

kml-open-dialog-title = Abrir KML ou KMZ

kml-save-dialog-title = Salvar KML ou KMZ como

screenshot-empty-error = Nada para salvar ainda.

kml-drop-type-error = Só é possível soltar arquivos .kml ou .kmz aqui.

globe-placemark-action = 📍 Adicionar local aqui

globe-fly-to-action = ✈ Voar até aqui

globe-ruler-point-action = 📏 Adicionar ponto na régua

globe-copy-coordinates-action = 📋 Copiar coordenadas

globe-context-menu-hint = Clique sobre o globo.

app-ready-status = Pronto. Arraste para girar, role para aproximar.

search-empty-status = Nenhum resultado encontrado.

search-pending-status = Buscando…

search-input-placeholder = Buscar lugar, endereço ou “lat, lon”

search-submit-action = 🔍 Buscar

settings-open-tooltip = Configurações

navigation-compass-tooltip = Clique para alinhar ao norte

navigation-north-tooltip = Norte para cima

navigation-reverse-tooltip = Girar a vista 180°

navigation-rotate-left-tooltip = Girar para a esquerda (segure)

navigation-rotate-right-tooltip = Girar para a direita (segure)

navigation-pan-north-tooltip = Mover para o norte (segure)

navigation-pan-south-tooltip = Mover para o sul (segure)

navigation-pan-west-tooltip = Mover para oeste (segure)

navigation-pan-east-tooltip = Mover para leste (segure)

navigation-zoom-in-tooltip = Aproximar (segure)

navigation-zoom-out-tooltip = Afastar (segure)

navigation-zoom-slider-tooltip = Arraste para ajustar o zoom

settings-title = ⚙ Configurações

settings-text-section-title = Tamanho do texto

settings-ui-scale-label = Botões e painéis

settings-map-scale-label = Texto do mapa (nomes, régua, escala)

settings-folders-section-title = Onde salvar os arquivos

settings-folder-browse-action = 📁 Procurar…

settings-screenshot-folder-label = Capturas de tela (PNG):

settings-screenshot-folder-dialog-title = Pasta das capturas

settings-kml-folder-label = Arquivos KML/KMZ:

settings-kml-folder-dialog-title = Pasta dos KML/KMZ

settings-folder-creation-help = As pastas são criadas automaticamente ao salvar.

settings-reset-action = Restaurar padrões

settings-cache-section-title = Cache do mapa
settings-cache-clear-action = Limpar cache em disco
settings-cache-clear-help = Remove os tiles baixados do mapa. Eles serão baixados novamente quando necessário.
settings-cache-clearing-label = Limpando cache…
settings-cache-cleared-status = Cache de tiles em disco limpo.
settings-cache-clear-error = Could not clear on-disk cache: { $error }
settings-cache-disk-usage = { $size } em disco
settings-cache-measuring-label = Calculando…
settings-cache-usage-unavailable = Cache size unavailable

layers-section-title = Camadas

wayback-loading-status = ⏳ Carregando datas disponíveis…

wayback-older-tooltip = Mais antiga

wayback-newer-tooltip = Mais recente

wayback-fallback-help = Nem todo lugar tem imagem em todas as datas; áreas sem imagem mostram a versão mais próxima disponível.

layers-grid-toggle = Grade de latitude/longitude

layers-country-borders-toggle = Limites dos países (mundo todo)

layers-state-borders-toggle = Estados / províncias (mundo todo)

layers-place-labels-toggle = Nomes de países, estados e cidades

layers-municipal-borders-toggle = Limites de municípios do mundo (OSM) e Brasil (IBGE)

layers-metro-borders-toggle = Regiões metropolitanas (disponível apenas para o Brasil)

boundaries-loading-status = ⏳ Carregando limites e nomes…

layers-atmosphere-toggle = Atmosfera e estrelas

layers-sunlight-toggle = Luz do sol em tempo real (dia/noite)

layers-auto-rotation-toggle = Girar automaticamente

layers-flights-toggle = Aviões reais (OpenSky + ADSB.lol)
aircraft-summary-status = Aviões reais rastreados: { $count } | OpenSky: { $global } | ADSB.lol: { $local }
aircraft-loading-status = carregando
aircraft-zoom-in-status = aproxime o mapa
aircraft-updated-status = há { $minutes } min
aircraft-source-error = { $source }: { $error }
aircraft-hover-tooltip = { $code }  (ICAO { $hex })
    Altitude: { $altitude }  |  Velocidade: { $speed }
    Rumo: { $heading }  |  Posição há { $age } s
layers-weather-toggle = Mapa do clima (radar de chuva ao vivo)
layers-clouds-toggle = Mapa das nuvens por satélite (Matt Eason)
layers-live-overlays-help = Aviões reais, com cobertura incompleta: atualização global a cada ~17 min e local a cada ~90 s. Nuvens ~3 h; radar de chuva ~10 min. Requer internet.

navigation-section-title = Navegação

navigation-reset-north-action = 🧭 Norte para cima

navigation-global-view-action = 🌐 Visão global

search-results-label = Resultados da busca:

placemarks-section-title = Locais (Placemarks)

placemarks-name-placeholder = Nome

placemarks-add-center-action = ➕ No centro

placemarks-fly-to-action = Ir

kml-drop-zone-hint = 📥 Arraste um arquivo .kml ou .kmz aqui

kml-save-action = 💾 Salvar

kml-save-tooltip = Salva na pasta de KML/KMZ (veja Configurações)

kml-save-as-action = 💾 Salvar como…

kml-open-action = 📂 Abrir KML/KMZ…

ruler-section-title = Régua

ruler-enable-toggle = Medir distâncias (clique no globo)

ruler-clear-action = Limpar régua

export-section-title = Exportar

screenshot-save-action = 📷 Salvar captura (PNG)

settings-folders-action = ⚙ Mudar pastas…

controls-help-toggle = Mostrar ajuda de controles

controls-help-text = • Arrastar (esq.): girar o globo
    • Botão direito/Shift+arrastar: girar a vista
    • Roda/pinça ou +/-: zoom
    • Duplo clique: voar até o ponto
    • Setas: mover   • N: norte para cima
    • Botão direito (clique): menu de contexto

placemarks-fallback-name = Sem nome

provider-satellite-label = Satélite (Esri World Imagery)

provider-wayback-label = Imagens históricas (Esri Wayback)

provider-streets-label = Ruas (OpenStreetMap)

provider-topography-label = Relevo/Topografia (OpenTopoMap)

provider-imagery-attribution = Imagens © Esri, Maxar, Earthstar Geographics

provider-streets-attribution = © contribuidores do OpenStreetMap

# Whole messages: arguments retain the original numeric formatting.

kml-save-success = Salvo em { $path }

kml-save-error = Erro ao salvar: { $error }

kml-import-success =
    { $count ->
        [1] { $count } local importado.
       *[other] { $count } locais importados.
    } — { $path }

kml-open-error = Erro ao abrir: { $error }

screenshot-save-success = Captura salva: { $path }

screenshot-save-error = Erro: { $error }

search-navigation-status = Indo para { $destination }

search-results-status =
    { $count ->
        [1] { $count } resultado encontrado.
       *[other] { $count } resultados encontrados.
    }

search-request-error = Erro na busca: { $error }

wayback-snapshot-attribution = { $attribution } — imagem de { $date }

placemarks-generated-name = Local { $number }

wayback-availability-status = { $count } versões disponíveis

export-folder-label = Pasta: { $path }

ruler-measurement-summary = Pontos: { $count }   Total: { $distance }

statusbar-tiles-loading = ⏳ carregando { $count } tiles

statusbar-camera-center = Centro: { $latitude }°, { $longitude }°

statusbar-camera-altitude = Altitude da câmera: { $distance }


settings-uncap-fps-toggle = FPS sem limite (requer reiniciar)
settings-frame-sync-help = Por padrão, a apresentação é sincronizada com a taxa de atualização da tela. G-SYNC/FreeSync é configurado na tela e no driver gráfico.

# Explorer / Visualização e novas camadas — conteúdo bilíngue
top-explore-action = Explorar +
top-earth = Terra
top-moon = Lua
top-custom-planet = Planeta customizável
explorer-title = Explorar · locais, céu e planetas
explorer-selected-location = Local selecionado
explorer-search-place = Pesquise um local para ver informações
explorer-latlon = Latitude { $latitude } · Longitude { $longitude }
explorer-center = Centro: { $latitude }, { $longitude }
explorer-place-photos = Fotos do local
explorer-photos-opened = Galeria de fotos aberta no navegador
explorer-view-osm = Detalhes no OpenStreetMap
explorer-map-opened = Mapa aberto no navegador
explorer-photo-copyright = As fotos são pesquisadas no Wikimedia Commons e seus direitos pertencem aos autores. Os detalhes são exibidos no navegador, não são avaliações do Google Maps.
explorer-narrated-tour = Passeio narrado (TTS)
explorer-tour-default = Estamos explorando { $place }. Explore o local no globo e escolha pontos de interesse para continuar seu passeio.
explorer-read-aloud = ▶ Narrar texto
explorer-speech-sent = Texto enviado ao sintetizador de voz
explorer-globe-layers = Camadas no globo
explorer-layer-info = As camadas georreferenciadas são exibidas diretamente no globo com dados on-line (requer internet).
explorer-cloud-credit = Imagens de nuvens: Matt Eason / Live Cloud Maps
explorer-cloud-source = Live Cloud Maps — autor e código-fonte
explorer-cloud-license = Contém dados modificados da EUMETSAT · CC0 1.0
explorer-radar-credit = Fonte do radar: RainViewer
explorer-radar-source = Radar meteorológico RainViewer
explorer-radar-explanation = Verde/amarelo/vermelho indicam a intensidade aproximada da chuva (refletividade do radar), não a temperatura infravermelha do satélite. A cobertura e atualização variam.
explorer-bodies = Corpos celestes
explorer-earth = 🌍 Terra
explorer-moon = 🌙 Lua
explorer-custom-planet = 🪐 Planeta customizável
explorer-moon-fixed = A Lua é fixa e não editável. As imagens abaixo pertencem exclusivamente ao planeta customizável.
explorer-name = Nome:
explorer-image = Imagem:
explorer-browse = Escolher
explorer-display-planet = Mostrar imagem do planeta
explorer-planet-displayed = Planeta exibido no globo
explorer-save-planet = Salvar planeta…
explorer-load-planet = Carregar planeta…
explorer-planet-loaded = Planeta e céu carregados no globo
explorer-planet-saved = Planeta e céu incorporados em { $path }
explorer-file-working = Processando arquivo…
explorer-embedded-image = Imagem incorporada
explorer-sky-galaxy = Céu e galáxia personalizados
explorer-image-path = Caminho da imagem
explorer-display-galaxy = Exibir galáxia
explorer-sky-applied = Céu aplicado ao fundo do globo
explorer-sky-preview = O céu personalizado é pré-visualizado aqui; não é usado como skybox no renderizador da Terra.
live-clouds-failed = Falha ao atualizar nuvens
live-cached-image =  (exibindo imagem em cache)
live-clouds-ok = Nuvens: Matt Eason (atualização a cada 3 horas)
live-clouds-loading = Nuvens: carregando imagem de satélite…
live-radar-failed = Falha ao atualizar radar
live-radar-age = Radar de chuva: observação de aproximadamente { $minutes } min atrás
live-radar-loading = Radar de chuva: carregando dados do RainViewer…
sidebar-cloud-source = Dados de nuvens: Matt Eason / EUMETSAT
sidebar-radar-source = Dados de radar: RainViewer
live-cloud-credit = Matt Eason · Live Cloud Maps · Contém dados modificados da EUMETSAT
live-radar-credit = Radar de precipitação: RainViewer (rainviewer.com)

# Native desktop menus
menu-file-title = Arquivo
menu-kml-open = Abrir KML/KMZ…
menu-kml-save = Salvar KML/KMZ
menu-kml-save-as = Salvar KML/KMZ como…
menu-screenshot-save = Salvar captura de tela
menu-quit-action = Sair do EarthCraft

sidebar-hide-tooltip = Ocultar painel lateral
sidebar-show-tooltip = Mostrar painel lateral

settings-distance-units-label = Unidades de distância
settings-units-system = Localidade do sistema
settings-units-metric = Quilômetros / metros
settings-units-imperial = Milhas / pés

ruler-unit-automatic = Automático
ruler-unit-centimeters = Centímetros
ruler-unit-meters = Metros
ruler-unit-kilometers = Quilômetros
ruler-unit-inches = Polegadas
ruler-unit-feet = Pés
ruler-unit-yards = Jardas
ruler-unit-miles = Milhas
ruler-unit-nautical-miles = Milhas náuticas
ruler-unit-smoots = Smoots
ruler-unit-degrees = Graus
ruler-unit-arcseconds = Segundos de arco
ruler-unit-football-fields = Campos de futebol
ruler-unit-american-football-fields = Campos de futebol americano
ruler-units-label = Unidades
ruler-distance-smoots = { $value } smoots
ruler-distance-football-fields = { $value } campos de futebol
ruler-unit-automatic-help = Usa as unidades de distância selecionadas nas Configurações.
ruler-unit-centimeters-help = 1 centímetro = 0,01 metro.
ruler-unit-meters-help = 1 metro = 100 centímetros.
ruler-unit-kilometers-help = 1 quilômetro = 1.000 metros.
ruler-unit-inches-help = 1 polegada = 2,54 centímetros.
ruler-unit-feet-help = 1 pé = 12 polegadas.
ruler-unit-yards-help = 1 jarda = 3 pés.
ruler-unit-miles-help = 1 milha = 1.760 jardas.
ruler-unit-nautical-miles-help = 1 milha náutica = 1.852 metros.
ruler-unit-smoots-help = 1 smoot = 67 polegadas.
ruler-unit-degrees-help = Ângulo central ao longo do trajeto medido no globo.
ruler-unit-arcseconds-help = 1 segundo de arco = 1/3.600 de grau de ângulo central.
ruler-unit-football-fields-help = Um campo de futebol americano equivale a 100 jardas, sem as zonas de pontuação.

altitude-edit-tooltip = Definir a altitude exata da câmera
altitude-edit-title = Altitude da câmera
altitude-apply-action = Aplicar
altitude-cancel-action = Cancelar
altitude-input-error = Digite um número entre { $minimum } e { $maximum } { $unit }.

settings-hide-navigation-controls-toggle = Ocultar controles de navegação

settings-close-action = Fechar

ruler-point-edit-help = Alt+clique: novo trajeto · Arraste para mover · Cursor + Delete: remover
ruler-point-edit-help-macos = Option+clique: novo trajeto · Arraste para mover · Cursor + Delete: remover
compass-north = N
compass-northeast = NE
compass-east = L
compass-southeast = SE
compass-south = S
compass-southwest = SO
compass-west = O
compass-northwest = NO
compass-heading = { $degrees }° { $direction }
compass-heading-tooltip = Direção no topo do mapa, no sentido horário a partir do norte verdadeiro. Clique para alinhar ao norte.
layers-grid-numbers-toggle = Mostrar números
ruler-bearing-label = Rumo do último segmento: { $bearing }
ruler-bearing-tooltip = Rumo inicial do círculo máximo do penúltimo ao último ponto, no sentido horário a partir do norte verdadeiro.
grid-coordinate-label = { $degrees }° { $direction }

menu-edit-title = Editar
edit-undo-action = Desfazer
edit-redo-action = Refazer

ruler-points-edit-action = Editar coordenadas…
ruler-points-title = Coordenadas da régua
ruler-path-heading = Trajeto { $number }
ruler-point-row = { $number }: { $latitude }°, { $longitude }°
ruler-point-edit-mode = Editar ponto selecionado
ruler-point-add-mode = Adicionar um ponto
ruler-point-latitude = Latitude (°)
ruler-point-longitude = Longitude (°)
ruler-point-new-path-toggle = Iniciar trajeto separado
ruler-point-input-help = Graus decimais. Latitude negativa indica sul; longitude negativa indica oeste.
ruler-point-apply-action = Aplicar
ruler-point-add-action = Adicionar ponto
ruler-point-new-action = Novo ponto
ruler-point-close-action = Fechar

settings-earth-model-label = Modelo da Terra
earth-model-sphere = Esfera perfeita
earth-model-wgs84 = Realista — WGS 84 (G2296)
earth-model-sphere-help = Esfera original de raio médio. Alternativa opcional ao modelo realista da Terra.
earth-model-wgs84-help = Modelo padrão da Terra, com geometria e geodésicas no elipsoide WGS 84. Referencial G2296: EPSG:10606 (2D), EPSG:10605 (3D); sem transformações de época, movimento tectônico, relevo ou alturas geoidais. Usa o renderizador de globo na CPU.

places-root-name = Meus lugares
places-document-name = Documento
places-folder-name = Nova pasta
places-add-folder-action = + Pasta
places-clear-all-action = Limpar tudo
places-delete-action = Excluir
places-visible-help = Mostrar este lugar e seus filhos no globo
places-empty-help = Abra um arquivo KML ou KMZ ou adicione um lugar. Arraste lugares para pastas para organizá-los.
places-description-placeholder = Descrição

# Worldwide layers and the 3D Easter egg (merged)
flat-earth-title = ✦ TERRA PLANA SECRETA
flat-earth-unlocked = Easter egg Terra Plana desbloqueado! Esc para voltar.
flat-earth-disclaimer = Uma brincadeira visual, não um modelo científico.
flat-earth-hint = Arraste: girar · Shift: mover · Roda: zoom · Duplo clique: reiniciar · Esc: sair
flat-earth-return = ↩ Voltar ao globo
flat-earth-image-error = Não foi possível carregar a ilustração secreta.
layers-vessels-toggle = Navios AIS do mundo inteiro
layers-satellites-toggle = Satélites (órbitas CelesTrak, estimadas)
layers-traffic-toggle = Congestionamento real (TomTom; chave opcional)
layers-traffic-key = Chave de API TomTom (não é salva)
layers-roads-toggle = Estradas do mundo (OSM, sem chave)
