pkgname=wirepost-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="Lean CLI for sending SMTP mail with templating, retries, and DKIM"
arch=('x86_64')
url="https://github.com/alex/wirepost"
license=('custom')
provides=('wirepost')
conflicts=('wirepost' 'wirepost-git')
depends=('gcc-libs' 'openssl' 'ca-certificates')
source=("wirepost-linux-x86_64.tar.gz::https://github.com/alex-oleshkevich/wirepost/releases/download/v${pkgver}/wirepost-linux-x86_64.tar.gz")
noextract=('wirepost-linux-x86_64.tar.gz')
sha256sums=('SKIP')

package() {
  cd "${srcdir}"
  tar -xzf wirepost-linux-x86_64.tar.gz
  install -Dm755 wirepost-linux-x86_64/wirepost "${pkgdir}/usr/bin/wirepost"
}
