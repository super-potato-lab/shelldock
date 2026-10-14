import type { Page } from '@playwright/test';
// Test-only IPC bridge. Documentation IPs and synthetic files; never included in the app.
export async function installDesktopFixture(page: Page, connectionCount = 28) {
  await page.addInitScript(({connectionCount}) => {
    const host = window as unknown as Record<string, any>;
    const connections = Array.from({length:connectionCount},(_,i)=>({id:'demo-'+i,name:i?'demo-node-'+(i+1):'demo-node',host:'192.0.2.'+(i+10),port:22,username:'demo',group:i%2?'测试环境':'开发环境',auth:'password',privateKeyPath:null,notes:'',hasPassword:true,protocol:'ssh',extra:{}}));
    let callbackId = 0; let sample = 0;
    const callbacks = new Map<number, (value:unknown)=>void>();
    const requests: {command:string;args:Record<string,any>}[]=[];
    host.__testRequests = requests;
    host.isTauri = true;
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener:()=>{}};
    const entry = (name:string,path:string,isDir=true)=>({name,path,isDir,isSymlink:false,size:isDir?0:4096,modified:1791936000,permissions:isDir?0o40755:0o100644,uid:1000,gid:1000});
    host.__TAURI_INTERNALS__ = {
      transformCallback:(callback:(value:unknown)=>void)=>{const id=++callbackId;callbacks.set(id,callback);return id;},
      unregisterCallback:(id:number)=>callbacks.delete(id),
      invoke:async(command:string,args:Record<string,any>={})=>{
        requests.push({command,args});
        if(command==='bootstrap')return {connections,history:['pwd','uname -a'],quickCommands:['ls -lah','df -h','free -m']};
        if(command==='plugin:event|listen')return 1;
        if(command==='plugin:dialog|message')return true;
        if(command==='connect'){
          const onEvent=args.onEvent as {id:number};
          setTimeout(()=>callbacks.get(onEvent.id)?.({index:0,message:{event:'output',data:Array.from(new TextEncoder().encode('连接主机成功\r\nWelcome to demo Linux\r\ndemo@demo-node:~$ '))}}),30);
          return {status:'connected',sessionId:'session-'+args.id,path:'/home/demo'};
        }
        if(command==='list_files'){
          if(args.path==='/')return ['bin','boot','dev','etc','home','lib','media','mnt','opt','proc','root','run','sbin','srv','sys','tmp','usr','var'].map(name=>entry(name,'/'+name));
          if(args.path==='/home')return [entry('demo','/home/demo')];
          if(args.path==='/var')return [entry('log','/var/log')];
          if(args.path==='/home/demo')return ['.cache','.config','.local','.ssh','Documents'].map(name=>entry(name,args.path+'/'+name)).concat([entry('.bashrc',args.path+'/.bashrc',false),entry('notes.txt',args.path+'/notes.txt',false)]);
          return [entry('example.txt',args.path+'/example.txt',false)];
        }
        if(command==='sample_metrics'){
          sample++;
          return {system:'Linux demo-node',timestamp:1791936000+sample*3,uptime:10022400,cpuPercent:sample===1?null:sample%2?17:7,memoryTotal:1717986918,memoryAvailable:1182793728,swapTotal:2147483648,swapFree:2147483648,load:[.01,.02,.03],interfaces:[{name:'eth0',received:1048576,sent:2097152,receiveRate:sample===1?null:7200,sendRate:sample===1?null:51000}],disks:[{name:'tmpfs',mount:'/run',total:168820736,used:1048576,available:167772160},{name:'/dev/vda1',mount:'/',total:41875931136,used:12455405158,available:29420525978},{name:'tmpfs',mount:'/dev/shm',total:845152256,used:0,available:845152256}],processes:[{pid:123,user:'demo',cpu:1.2,memory:.4,command:'bash'}]};
        }
        if(command==='save_connection')return {...args.connection,id:args.connection.id||'saved',hasPassword:false};
        if(['terminal_input','terminal_ack','terminal_resize','record_command','disconnect','save_quick_commands','plugin:event|unlisten'].includes(command))return;
        throw new Error('Unmocked IPC command: '+command);
      }
    };
  },{connectionCount});
}
