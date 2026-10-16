FROM node:26.11.1-alpine3.24@sha256:143494b1da2945f061539253adc65e4f1569ddf07da2d384c022c791a9d90a4a

WORKDIR /app

COPY package*.json ./
RUN npm install

COPY . .
RUN npm run build

CMD ["npm", "start"]
