const { chromium } = require('/Users/zhenglizhi/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
(async()=>{
const b=await chromium.launch({channel:'chrome'});
const p=await b.newPage({viewport:{width:1280,height:900}});const errors=[];p.on('pageerror',e=>errors.push(e.message));
const url='http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html';
const assert=(v,m)=>{if(!v)throw Error(m)};
async function open(query=''){await p.goto(url+query);await p.locator('.trace-pagination').waitFor();await p.evaluate(()=>document.fonts.ready)}
await open();
assert(!(await p.locator('body').innerText()).includes('must-not-appear'),'user hidden');
const baseline=await p.locator('.trace-card').evaluate(e=>e.getBoundingClientRect().height);
await p.locator('.trace-edge-next').click();assert((await p.locator('.trace-pagination').innerText()).replace(/\s/g,'')==='1/4','4→1');
assert((await p.locator('.detail-panel').innerText()).includes('Key A'),'first key');
await p.locator('.trace-edge-prev').click();assert((await p.locator('.trace-pagination').innerText()).replace(/\s/g,'')==='4/4','1→4');
for(let i=0;i<4;i++){await p.locator('.trace-edge-next').click();const height=await p.locator('.trace-card').evaluate(e=>e.getBoundingClientRect().height);assert(Math.abs(height-baseline)<2,'constant card height')}
await p.mouse.move(0,0);await p.waitForTimeout(180);assert(await p.locator('.trace-edge-next span').evaluate(e=>getComputedStyle(e).opacity)==='0','hidden default');
await p.locator('.trace-edge-next').hover();await p.waitForTimeout(180);assert(await p.locator('.trace-edge-next span').evaluate(e=>getComputedStyle(e).opacity)==='1','hover');
await p.locator('.trace-edge-next').focus();await p.keyboard.press('ArrowRight');assert((await p.locator('.trace-pagination').innerText()).replace(/\s/g,'')==='1/4','keyboard');
await p.screenshot({path:'output/request-detail-developed.png',fullPage:true});
const geometry=[];
for(const width of [900,1024,1440,1920,390]){await p.setViewportSize({width,height:width===900?640:900});const overflows=await p.evaluate(()=>[...document.querySelectorAll('body *')].filter(e=>e.clientWidth>0&&e.getClientRects().length&&e.scrollWidth>e.clientWidth+2&&getComputedStyle(e).overflowX!=='hidden').map(e=>({tag:e.tagName,class:e.className,overflow:e.scrollWidth-e.clientWidth})));assert(overflows.length===0,JSON.stringify({width,overflows}));geometry.push({width,card:await p.locator('.trace-card').evaluate(e=>Math.round(e.getBoundingClientRect().height))})}
await p.setViewportSize({width:1280,height:900});await open('?count=1');assert(await p.locator('.trace-edge').count()===0,'single no arrows');assert((await p.locator('.trace-pagination').innerText()).replace(/\s/g,'')==='1/1','single count');assert(Math.abs(await p.locator('.trace-card').evaluate(e=>e.getBoundingClientRect().height)-baseline)<2,'single same layout');
await open('?theme=dark&status=success&locale=en-US');await p.waitForTimeout(300);assert((await p.locator('.final-status').innerText()).includes('succeeded'),'english');await p.screenshot({path:'output/request-detail-developed-dark.png',fullPage:true});assert(await p.locator('.error-block').count()===0,'success not error');
await open('?long=1');await p.locator('.extra-toggle').click();assert((await p.locator('.extra-block').innerText()).includes('diagnostic-'),'extra diagnostic retained');assert(errors.length===0,errors.join('\n'));
console.log(JSON.stringify({passed:true,geometry}));await b.close();
})().catch(e=>{console.error(e);process.exit(1)});
