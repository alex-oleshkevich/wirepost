pkgname=mailer-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="Lean CLI for sending SMTP mail with templating, retries, and DKIM"
arch=('x86_64')
url="https://github.com/alex/mailer"
license=('custom')
provides=('mailer')
conflicts=('mailer' 'mailer-git')
depends=('gcc-libs' 'openssl' 'ca-certificates')
source=("mailer-linux-x86_64.tar.gz::https://github.com/alex/mailer/releases/download/v${pkgver}/mailer-linux-x86_64.tar.gz")
noextract=('mailer-linux-x86_64.tar.gz')
sha256sums=('SKIP')

package() {
  cd "${srcdir}"
  tar -xzf mailer-linux-x86_64.tar.gz
  install -Dm755 mailer-linux-x86_64/mailer "${pkgdir}/usr/bin/mailer"
}
