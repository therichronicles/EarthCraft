# Navegação por controles

[English](CONTROLLERS.md) | [Português (Brasil)](CONTROLLERS.pt-BR.md)

Configurações → Controles permite usar gamepads/joysticks reconhecidos pelo
[gilrs 0.11.2](https://docs.rs/gilrs/0.11.2/gilrs/), com conexão sem reiniciar.
Selecione o dispositivo, ajuste o perfil Explorar, acompanhe entradas ao vivo e
restaure os padrões. Mouse e teclado continuam disponíveis.

O analógico esquerdo move o globo; o eixo X direito gira e o eixo Y inclina a câmera; gatilhos
esquerdo/direito afastam/aproximam. Sul adiciona um marcador no centro, Leste
cancela movimento/arraste, Norte redefine orientação e inclinação, Oeste alterna medições,
Select abre Lugares e Start abre Configurações. Os nomes seguem posições
normalizadas, sem pressupor marca de console. A inclinação aplica-se apenas à Terra.

Cada ação de navegação permite escolher um dos seis eixos ou desativá-la, com
zona morta, inversão, sensibilidade e curva independentes. Eixos compartilhados
são permitidos explicitamente. Botões reconhecidos podem receber qualquer ação.
Pressionar para vincular aguarda dez segundos, permite cancelar, exige passagem
pelo neutro antes de capturar um eixo e confirma a substituição de botão ocupado.

O movimento usa tempo limitado e só é aplicado uma vez por frame. A Terra usa o
modelo selecionado e outros corpos usam a órbita existente. Ações reutilizam o
mesmo dispatcher dos menus. Desconectar libera entradas. Edição de texto,
diálogos, popups, janela sem foco e reinicialização suspendem movimento; solte
eixos/botões antes de retomar. A consulta limitada permite detectar conexão com
o globo parado.

Perfis locais em settings.json usam UUID do modelo, fabricante/produto e nome.
Modelos diferentes mantêm perfis independentes; modelos idênticos sem identidade
única compartilham perfil. Não é identificação por número de série. Perfis
inválidos usam padrões independentes e números são limitados.

Linux exige libudev e arquivos de desenvolvimento; FreeBSD usa libudev-devd,
libinotify e acesso evdev/webcamd. Snap solicita joystick e Flatpak, dispositivos
de entrada. Permissões podem exigir configuração. Erros aparecem nas
Configurações. Compilar no macOS não comprova pacotes ou funcionamento físico.

Studio/voo, HOTAS/HID bruto, SpaceMouse, vibração e importação/exportação de perfis
ficam para trabalho posterior. Os testes exercitam zona morta, inversão/curvas,
bordas/liberação de botões, conflitos, passos limitados e persistência por modelo.

O lockfile mantém libc 0.2.189: o CI do FreeBSD encontrou uma referência de
nix 0.31.3 a NOTE_PCTRLMASK, removida em libc 0.2.190. Reavaliar quando
nix/gilrs oferecer uma atualização compatível. Não há alteração de código da
dependência nem código unsafe no aplicativo.

A inclinação varia de 0° (nadir) a 85° em direção ao horizonte sem mover a câmera.
Arraste com o botão central ou use o controle do painel Globo. R redefine norte e
inclinação; N redefine apenas orientação. Voar até um ponto retorna ao nadir.
A vista inicial salva mantém a inclinação. Perfis antigos mantêm vínculos e
recebem inclinação no eixo Y direito. O monitor usa linhas fixas para botões.

No macOS, relatórios completos do Nintendo Pro Controller (0x30/0x31,
modelo 057e:2009) precisam de um decodificador adicional: a entrada HID padrão
do gilrs não interpreta esses dados específicos do fabricante. Um leitor hidapi
não exclusivo, em segundo plano, envia eventos normalizados para a mesma fila
gilrs. Seleção, perfis, vínculos, foco e movimento continuam no adaptador existente.
Relatórios HID padrão e outros modelos mantêm a entrada nativa do gilrs.
O leitor solicita a calibração de fábrica/usuário por comandos SPI somente de
leitura; dados inválidos/indisponíveis usam o intervalo completo de 12 bits.
Não altera modo de relatório, memória flash ou ativa vibração. Eixos são
agrupados e transições de botões usam uma fila limitada. Excesso de entradas,
desconexão ou interrupção dos relatórios libera os controles e exige retorno ao
neutro. Erros aparecem nas Configurações. Esse caminho aceita um Pro Controller
conectado por vez: o gilrs não expõe o caminho HID necessário para associar
múltiplos dispositivos idênticos com segurança.

O teste físico no macOS verificou quatro eixos, um botão frontal, retorno ao
neutro e movimento de navegação pelo adaptador de produção. O globo interativo e
outras plataformas ainda exigem a lista de verificações do aplicativo.
Para repetir no macOS, conecte um Pro Controller e execute
`cargo test -p earthcraft --locked nintendo_hardware_input -- --ignored --nocapture`.
Centralize os analógicos, mova ambos completamente em todas as direções,
segure um botão frontal brevemente e solte tudo. O teste termina ao passar
ou após dez minutos.
