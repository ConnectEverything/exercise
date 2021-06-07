FROM scratch
ADD server3.conf /
RUN ./nats-server -addr 10.20.20.3 -c ./server3.conf -js -sd storage
