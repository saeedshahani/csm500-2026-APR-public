import {useEffect,useRef} from 'react';
import {EditorState} from '@codemirror/state';
import {EditorView,lineNumbers,highlightActiveLine,drawSelection} from '@codemirror/view';
export function Editor({value,onChange}:{value:string;onChange:(text:string)=>void}) {
  const host=useRef<HTMLDivElement>(null),view=useRef<EditorView|null>(null),callback=useRef(onChange);
  useEffect(()=>{callback.current=onChange;},[onChange]);
  useEffect(()=>{
    if(!host.current)return;
    const editor=new EditorView({parent:host.current,state:EditorState.create({doc:value,extensions:[lineNumbers(),highlightActiveLine(),drawSelection(),EditorView.lineWrapping,EditorView.contentAttributes.of({'aria-label':'Program source','spellcheck':'false'}),EditorView.updateListener.of(update=>{if(update.docChanged)callback.current(update.state.doc.toString());})]})});view.current=editor;
    return ()=>{editor.destroy();view.current=null;};
    // Initial content is followed by the controlled update below.
  },[]);
  useEffect(()=>{const editor=view.current;if(editor&&editor.state.doc.toString()!==value)editor.dispatch({changes:{from:0,to:editor.state.doc.length,insert:value}});},[value]);
  return <div className="source-editor" ref={host}/>;
}
