import {initializeApp,deleteApp} from 'firebase/app';
import {initializeAuth,inMemoryPersistence,createUserWithEmailAndPassword,signInWithEmailAndPassword,
  sendEmailVerification,sendPasswordResetEmail,signOut} from 'firebase/auth';
import {hostedConfig} from './hosted-config';
import type {EmulatorSession} from './emulator-session';

function context(){
  const app=initializeApp(hostedConfig,`hosted-${crypto.randomUUID()}`);
  const auth=initializeAuth(app,{persistence:inMemoryPersistence});let closed=false;
  return {auth,async close(){if(closed)return;closed=true;try{await signOut(auth);}finally{await deleteApp(app);}}};
}
export async function hostedSession(email:string,password:string,create:boolean):Promise<EmulatorSession>{
  const c=context();
  try{
    const result=await (create?createUserWithEmailAndPassword:signInWithEmailAndPassword)(c.auth,email,password);
    if(create){await sendEmailVerification(result.user);throw Error('Account created. Check your email and verify the address, then sign in.');}
    if(!result.user.emailVerified)throw Error('Verify your email before signing in. Use Resend verification if you need another email.');
    return {uid:result.user.uid,token:()=>result.user.getIdToken(),close:c.close};
  }catch(e){await c.close();throw e;}
}
export async function accountEmail(email:string,password:string,kind:'reset'|'verify'){
  const c=context();
  try{
    if(kind==='reset')await sendPasswordResetEmail(c.auth,email);
    else{const result=await signInWithEmailAndPassword(c.auth,email,password);if(!result.user.emailVerified)await sendEmailVerification(result.user);}
  }finally{await c.close();}
}
