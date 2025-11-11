pkgname=mailer-git
pkgver=0.1.0
pkgrel=1
pkgdesc="Lean CLI for sending SMTP mail with templating, retries, and DKIM"
arch=('x86_64')
_repo_url=$(git config --get remote.origin.url 2>/dev/null)
if [[ -z "$_repo_url" ]]; then
  _repo_url="https://github.com/alex/mailer.git"
fi
url="${_repo_url%.git}"
license=('custom')
provides=('mailer')
conflicts=('mailer')
depends=('gcc-libs' 'openssl' 'ca-certificates')
makedepends=('cargo' 'git')
source=("${pkgname}::git+${_repo_url}")
sha256sums=('SKIP')

pkgver() {
  cd "${srcdir}/${pkgname}"
  printf "0.1.0.r%s.g%s" \
    "$(git rev-list --count HEAD)" \
    "$(git rev-parse --short HEAD)"
}

build() {
  cd "${srcdir}/${pkgname}"
  cargo build --release --locked
}

package() {
  cd "${srcdir}/${pkgname}"
  install -Dm755 "target/release/mailer" "${pkgdir}/usr/bin/mailer"
  install -Dm644 README.md "${pkgdir}/usr/share/doc/${pkgname}/README.md"
  if [[ -f LICENSE ]]; then
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
  fi
}
