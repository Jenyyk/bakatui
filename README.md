## bakatui
TUI aplikace pro rozvrh na bakalářích

### Instalace
Naklonuj a zbuildi s [cargem](https://rustup.rs/):
```sh
git clone https://github.com/Jenyyk/bakatui.git
cd bakatui
cargo build --release
# Output binary bude v ./target/release/bakatui
```

### Login/Credentials
Na prvnim spuštění se aplikace zeptá na url ke škole a přihlašovací údaje, které si pak uloží do `$HOME/.config/bakatui/config`. Údaje jsou uložené v plaintextu. Komu to vadí tak může submitnout pull request.  
  

**TODO**  
- [ ] Uložit údaje do keyringu
- [ ] Přísahal bych že ještě něco, ale zapomněl jsem
