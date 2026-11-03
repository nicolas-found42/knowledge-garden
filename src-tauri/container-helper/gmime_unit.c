#include <gmime/gmime.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned index_ = 0, wanted; static long long skip; static int emitted = 0; static unsigned warning_count = 0;
static void warning(gint64 offset, GMimeParserWarning code, const gchar *item, gpointer data) {
 (void)item; (void)data; if(warning_count++ < 100) printf("WARN %lld %d\n",(long long)offset,(int)code);
 else if(warning_count == 101) printf("WARN -1 -1\n");
}
static void visit(GMimeObject *o, const char *path, unsigned depth) {
 if (emitted) return;
 if (depth > 24) { if (index_++ == wanted) { emitted=1; printf("SKIP %s nesting-limit\n",path); } return; }
 if (GMIME_IS_MULTIPART(o)) { GMimeMultipart *m=GMIME_MULTIPART(o); int count=g_mime_multipart_get_count(m); for(int i=0;i<count && !emitted;i++){char child[128]; if(snprintf(child,sizeof(child),"%s.%d",path,i+1)>=(int)sizeof(child)) return;visit(g_mime_multipart_get_part(m,i),child,depth+1);} return; }
 if (index_++ != wanted) return;
 emitted=1;
 if (!GMIME_IS_PART(o)) {printf("SKIP %s non-part\n",path);return;}
 GMimePart *part=GMIME_PART(o); GMimeContentType *ctype=g_mime_object_get_content_type(o);
 if (!g_mime_content_type_is_type(ctype,"text","plain") || g_mime_part_is_attachment(part)) {printf("SKIP %s unsupported\n",path);return;}
 GMimeDataWrapper *wrapper=g_mime_part_get_content(part); if(!wrapper) {printf("SKIP %s empty\n",path);return;}
 GMimeStream *source=g_mime_data_wrapper_get_stream(wrapper);g_mime_stream_reset(source);
 GMimeStream *filtered=g_mime_stream_filter_new(source);
 GMimeFilter *decode=g_mime_filter_basic_new(g_mime_data_wrapper_get_encoding(wrapper),FALSE);
 if(decode){g_mime_stream_filter_add(GMIME_STREAM_FILTER(filtered),decode);g_object_unref(decode);}
 const char *charset=g_mime_object_get_content_type_parameter(o,"charset");
 if(charset && g_ascii_strcasecmp(charset,"utf-8")!=0 && g_ascii_strcasecmp(charset,"us-ascii")!=0) {GMimeFilter *c=g_mime_filter_charset_new(charset,"UTF-8");if(!c){printf("SKIP %s charset\n",path);g_object_unref(filtered);return;}g_mime_stream_filter_add(GMIME_STREAM_FILTER(filtered),c);g_object_unref(c);}
 char b[65536];long long left=skip;while(left>0){size_t want=left>(long long)sizeof(b)?sizeof(b):(size_t)left;ssize_t n=g_mime_stream_read(filtered,b,want);if(n<=0){printf("END %s %lld\n",path,skip-left);g_object_unref(filtered);return;}left-=n;}
 long long offset=skip;
 for (;;) {
  ssize_t n=0; int eof=0;
  while(n<(ssize_t)sizeof(b)) { ssize_t got=g_mime_stream_read(filtered,b+n,sizeof(b)-(size_t)n); if(got<0){n=-1;break;} if(got==0){eof=1;break;} n+=got; }
  if(n<0){printf("ERROR %s\n",path);break;}
  if(n==0){printf("END %s %lld\n",path,offset);break;}
  char *enc=g_base64_encode((unsigned char*)b,n);
  printf("DATA %s %lld %lld %s\n",path,offset,(long long)n,enc);g_free(enc);
  if(fflush(stdout)!=0)break;
  offset+=n;
  if(eof){printf("END %s %lld\n",path,offset);break;}
 }
 g_object_unref(filtered);
}
int main(int argc,char **argv){if(argc!=4)return 2;wanted=(unsigned)strtoul(argv[2],NULL,10);skip=strtoll(argv[3],NULL,10);if(skip<0)return 2;g_mime_init();int fd=open(argv[1],O_RDONLY);if(fd<0)return 3;GMimeStream *s=g_mime_stream_fs_new(fd);GMimeParser *p=g_mime_parser_new_with_stream(s);g_mime_parser_set_persist_stream(p,TRUE);GMimeParserOptions *opts=g_mime_parser_options_new();g_mime_parser_options_set_warning_callback(opts,warning,NULL,NULL);GMimeMessage *m=g_mime_parser_construct_message(p,opts);g_mime_parser_options_free(opts);if(!m)return 4;visit(g_mime_message_get_mime_part(m),"1",0);if(!emitted)printf("DONE\n");g_object_unref(m);g_object_unref(p);g_object_unref(s);g_mime_shutdown();return 0;}
