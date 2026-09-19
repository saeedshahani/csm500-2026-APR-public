import guide from '../../docs/guides/workbench.md?raw';
// Render this repository-owned Markdown as React text, never as executable HTML.
export function Guide(){
 const lines=guide.split('\n');const blocks=[];
 for(let i=0;i<lines.length;i++){
  const line=lines[i];if(!line.trim())continue;
  if(line.startsWith('|')){const rows:string[][]=[];while(i<lines.length&&lines[i].startsWith('|')){if(!/^\|[-: |]+\|$/.test(lines[i]))rows.push(lines[i].split('|').slice(1,-1).map(s=>s.trim()));i++;}i--;blocks.push(<div className="table-scroll" key={i}><table><thead><tr>{rows[0].map((c,j)=><th key={j}>{c}</th>)}</tr></thead><tbody>{rows.slice(1).map((r,j)=><tr key={j}>{r.map((c,k)=><td key={k}>{c}</td>)}</tr>)}</tbody></table></div>);}
  else if(line.startsWith('# '))blocks.push(<h1 key={i}>{line.slice(2)}</h1>);
  else if(line.startsWith('## '))blocks.push(<h2 key={i}>{line.slice(3)}</h2>);
  else blocks.push(<p key={i}>{line}</p>);
 }
 return <article className="reader-guide"><button onClick={()=>window.print()}>Print guide</button>{blocks}</article>;
}
