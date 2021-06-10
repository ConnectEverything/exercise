FROM alpine
COPY --from=nats:2.2.6-alpine3.13 /usr/local/bin/nats-server /nats-server
COPY ./server3.conf /server3.conf
EXPOSE 4222 5000
ENTRYPOINT ["/nats-server"]
CMD ["--config", "/server3.conf", "-addr", "10.20.20.4", "-js", "-sd", "storage"]
