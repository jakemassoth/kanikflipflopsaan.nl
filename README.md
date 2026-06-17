# 🩴 kanikflipflopsaan.nl

### Kan ik flip flops aan?? 🩴☀️

For too long, humanity has suffered the single most agonizing daily dilemma known to the
lowlands: **can I wear my flip flops today, or not?** No more squinting at the sky. No more
sticking a hand out the window. No more regretful, soggy-toed walks home. This website answers
the question once and for all, with a glorious, decisive **JA!** or a sobering **Nee**.

It's the kind of problem you didn't know needed solving until it was solved. And now it is. You're
welcome. 🙌

## How it works ⚙️

1. You open the page and it asks the eternal question.
2. A tiny, blazingly-fast **Rust** serverless function wakes up, calls the
   [Open-Meteo](https://open-meteo.com) weather API using the **KNMI** model (the Dutch national
   weather institute — accept no substitutes), and pulls **today's forecast for De Bilt** 🇳🇱.
   Why De Bilt? Because it's smack in the middle of the country *and* it's literally where the
   KNMI's reference station lives. Poetic.
3. It runs the numbers through a ruthless four-factor flip-flop suitability algorithm.
4. It hands back a verdict — `ja` or `nee` — plus the full reasoning.
5. The page lights up: warm, sunny golds for **JA!**, or a moody grey-blue for **Nee**.

## The flip-flop algorithm 🧮

A day earns a **JA!** only if **all four** factors pass. We don't do half-measures here.

| Factor | What we check |
|--------|---------------|
| 🌡️ **Temperatuur** | The day's *maximum* temperature must clear the bar |
| 🌧️ **Neerslagkans** | The day's *peak* chance of rain must stay below the limit |
| 💨 **Wind** | Wind speed *and* gusts must be calm enough (nobody likes a flapping flip flop) |
| ☁️ **Weer** | No drizzle, rain, snow, or thunder spoiling the party |

And because the page is fully transparent, it shows you the **complete decision tree** — every
factor, with a green ✓ or a red ✗ — so you always know *exactly* why the answer is what it is.
No black boxes. Just honest, accountable footwear science. 🔬

## Pick your persona 🧑‍🤝‍🧑

Not everyone has the same relationship with cold toes. Choose the vibe that matches your soul:

- 🏄 **Surf Dude** — The laid-back beach soul. High tolerance. Will happily flip-flop through a
  brisk, drizzly 16°C and call it a lovely day.
- 🙂 **Gewoon mens** — The sensible default. A reasonable human with reasonable standards.
- 🥶 **Koukleum** — Perpetually, eternally cold. Demands real warmth, bone-dry skies, and barely a
  breath of wind before those slippers come anywhere near a foot.

Your choice is remembered for next time, because we care.

## Deliciously efficient caching 🦥

The weather doesn't change minute to minute, and neither should our answer. The function caches
its verdict on Vercel's Edge Network until **midnight, Amsterdam time** — so it serves up the same
crisp daily report all day long and only bothers the weather API **once per calendar day**, per
persona. Fast for you, kind to the free API. Everybody wins. 🎉

## Tech stack 🛠️

- **Rust** serverless function on the [Vercel Rust runtime](https://vercel.com/docs/functions/runtimes/rust)
- A single static `index.html` for the UI (no framework, no build step, just vibes)
- [Open-Meteo](https://open-meteo.com) KNMI model for the forecast
- [Nix flake](./flake.nix) for a reproducible dev environment

```
.
├── api/index.rs     # the Rust function: fetch weather → decide → {verdict, factoren}
├── index.html       # the page: persona dropdown, verdict, decision tree
├── Cargo.toml       # Rust deps + binary config
└── flake.nix        # reproducible Rust + Node dev shell
```

## Running it locally 🏃

Inside the Nix dev shell (`nix develop`, or let `direnv` load it for you):

```bash
# Compile the function
cargo build

# Run the whole thing — static page + function — together
npx vercel dev
# then open http://localhost:3000/
```

> Heads up: the midnight-aligned Edge caching only kicks in on a real Vercel deployment. Locally
> the function runs fresh every time, which is exactly what you want while hacking.

## Deploying 🚀

```bash
npx vercel deploy
```

Push to a connected Git repo for automatic deployments, or use the CLI above.

---

Made with ☀️ and an unreasonable enthusiasm for appropriate footwear. **Veel plezier met je
slippers!** 🩴
